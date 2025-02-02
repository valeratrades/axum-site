use chrono::{DateTime, Utc};
use futures::future::join_all;
use plotly::{Plot, Scatter, common::Line};
use v_exchanges::prelude::*;
use v_utils::prelude::*;

//TODO: once v_exchanges implements it properly, switch to take in any market
pub async fn try_build(limit: RequestRange, tf: Timeframe, market: AbsMarket) -> Result<Plot> {
	let mut exchange = market.client();
	//exchange.client_mut().update_default_option(BinanceOption::RequestConfig { recv_window: Some(5000) });
	exchange.set_max_tries(3);

	let exch_info = exchange.exchange_info(market).await.unwrap();
	let all_pairs = exch_info.usdt_pairs().collect::<Vec<Pair>>();

	let (normalized_df, dt_index) = collect_data(all_pairs.clone(), tf, limit, Arc::new(exchange)).await?;
	Ok(plotly_closes(normalized_df, dt_index, tf, market, &all_pairs))
}

pub async fn collect_data(pairs: Vec<Pair>, tf: Timeframe, range: RequestRange, exchange: Arc<Box<dyn Exchange>>) -> Result<(HashMap<Pair, Vec<f64>>, Vec<DateTime<Utc>>)> {
	//HACK: assumes we're never misaligned here
	let futures = pairs.into_iter().map(|symbol| {
		let exchange = Arc::clone(&exchange);
		async move {
			match get_historical_data(symbol, tf, range, exchange).await {
				Ok(series) => Ok((symbol, series)),
				Err(e) => {
					tracing::warn!("Failed to fetch data for symbol: {}. Error: {}", symbol, e);
					Err(e)
				}
			}
		}
	});

	let results = join_all(futures).await;
	let mut data: HashMap<Pair, Vec<f64>> = HashMap::new();
	let mut dt_index = Vec::new();

	results.into_iter().for_each(|result| {
		if let Ok((pair, series)) = result {
			if "BTCUSDT" == pair {
				dt_index = series.col_open_times;
			}
			data.insert(pair, series.col_closes);
		}
	});

	if dt_index.is_empty() {
		bail!("Failed to fetch data for BTCUSDT, aborting");
	}
	tracing::info!("Fetched data for {} pairs", data.len());

	let mut normalized_df: HashMap<Pair, Vec<f64>> = HashMap::new();
	for (symbol, closes) in data.into_iter() {
		let first_close: f64 = match closes.first() {
			Some(v) => *v,
			None => {
				eprintln!("Received empty data for: {symbol}");
				continue;
			}
		};
		let normalized = closes.into_iter().map(|p| (p / first_close).ln()).collect();
		normalized_df.insert(symbol, normalized);
	}

	let mut aligned_df = normalized_df.clone();
	for (key, closes) in normalized_df.iter() {
		if closes.len() != dt_index.len() {
			//HACK: maybe we want to fill the missing fields instead if there are not many of them
			eprintln!("misaligned: {key}");
			aligned_df.remove(key).unwrap();
		}
	}

	Ok((normalized_df, dt_index))
}

#[allow(unused)]
#[derive(Clone, Debug, Default, derive_new::new)]
pub struct RelevantHistoricalData {
	col_open_times: Vec<DateTime<Utc>>,
	col_opens: Vec<f64>,
	col_highs: Vec<f64>,
	col_lows: Vec<f64>,
	col_closes: Vec<f64>,
	col_volumes: Vec<f64>,
}
pub async fn get_historical_data(pair: Pair, tf: Timeframe, range: RequestRange, exchange: Arc<Box<dyn Exchange>>) -> Result<RelevantHistoricalData> {
	let klines = exchange.klines(pair, tf, range, exchange.source_market()).await?;

	let mut open_time = Vec::new();
	let mut open = Vec::new();
	let mut high = Vec::new();
	let mut low = Vec::new();
	let mut close = Vec::new();
	let mut volume = Vec::new();
	for k in klines {
		open_time.push(k.open_time);
		open.push(k.open);
		high.push(k.high);
		low.push(k.low);
		close.push(k.close);
		volume.push(k.volume_quote);
	}
	Ok(RelevantHistoricalData {
		col_open_times: open_time,
		col_opens: open,
		col_highs: high,
		col_lows: low,
		col_closes: close,
		col_volumes: volume,
	})
}

//TODO!!!: provide additional information: 1) BTCDOM, 2) average, 3) correlation, 4) volatility
pub fn plotly_closes(normalized_closes: HashMap<Pair, Vec<f64>>, dt_index: Vec<DateTime<Utc>>, tf: Timeframe, m: AbsMarket, all_pairs: &[Pair]) -> Plot {
	let mut performance: Vec<(Pair, f64)> = normalized_closes.iter().map(|(k, v)| (*k, (v[v.len() - 1] - v[0]))).collect();
	performance.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

	let n_samples = (performance.len() as f64).ln().round() as usize;
	let top: Vec<Pair> = performance.iter().rev().take(n_samples).map(|x| x.0).collect();
	let bottom: Vec<Pair> = performance.iter().take(n_samples).map(|x| x.0).collect();

	let mut plot = Plot::new();
	let hours = (dt_index.first().unwrap().signed_duration_since(dt_index.last().unwrap()) + tf.duration() * 1).num_hours().abs();
	let title = format!("Last {hours}h of {}/{} pairs on {m}", normalized_closes.len(), all_pairs.len());
	plot.set_layout(plotly::Layout::new().title(title));

	let mut add_trace = |name: Pair, width: f64, color: Option<&'static str>, legend: Option<String>| {
		let y_values: Vec<f64> = normalized_closes.get(&name).unwrap().to_owned();
		let x_values: Vec<String> = dt_index.iter().map(|dt| dt.to_rfc3339()).collect();

		let mut line = Line::new().width(width);
		if let Some(c) = color {
			line = line.color(c);
		}

		let mut trace = Scatter::new(x_values, y_values).mode(plotly::common::Mode::Lines).line(line);

		if let Some(l) = legend {
			trace = trace.name(&l);
		} else {
			trace = trace.show_legend(false);
		}
		plot.add_trace(trace);
	};

	let mut contains_btcusdt = false;
	for col_name in normalized_closes.keys() {
		if &col_name.to_string() == "BTCUSDT" {
			contains_btcusdt = true;
			continue;
		}
		if top.contains(col_name) || bottom.contains(col_name) {
			continue;
		}
		add_trace(*col_name, 1.0, Some("grey"), None);
	}
	let mut labeled_trace = |col_name: Pair, symbol: Option<&str>, line_width: f64, color: Option<&'static str>| {
		let p: f64 = performance.iter().find(|a| a.0 == col_name).unwrap().1;
		let symbol = match symbol {
			Some(s) => s,
			None => &col_name.to_string()[0..col_name.to_string().len() - 4].to_string().replace("1000", ""),
		};
		let sign = if p >= 0.0 { '+' } else { '-' };
		let change = format!("{:.2}", 100.0 * p.abs());
		let legend = format!("{:<5}{}{:>5}%", symbol, sign, change);
		add_trace(col_name, line_width, color, Some(legend));
	};
	for col_name in top.into_iter() {
		labeled_trace(col_name, None, 2.0, None);
	}
	if contains_btcusdt {
		labeled_trace("BTCUSDT".try_into().unwrap(), Some("~BTC~"), 3.5, Some("gold"));
	}
	for col_name in bottom.into_iter().rev() {
		labeled_trace(col_name, None, 2.0, None);
	}

	plot
}
