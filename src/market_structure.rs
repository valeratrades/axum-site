use std::{
	collections::HashMap,
	mem::transmute,
	path::Path,
	sync::{Arc, Mutex},
};

use chrono::{DateTime, Duration, Utc};
use color_eyre::eyre::{Result, eyre};
use futures::future::join_all;
use plotly::{Plot, Scatter, common::Line};
use serde::Deserialize;
use serde_json::Value;
use serde_with::{DisplayFromStr, serde_as};
use v_utils::trades::{Pair, Timeframe};

use crate::utils::deser_reqwest;

pub async fn try_build(spot_pairs_json_file: &Path) -> Result<Plot> {
	let json_content = std::fs::read_to_string(spot_pairs_json_file)?;
	let json_data: Value = serde_json::from_str(&json_content)?;
	let symbols: Vec<Pair> = json_data
		.as_array()
		.ok_or_else(|| eyre!("Expected an array in the JSON file"))?
		.iter()
		.map(|value| value.as_str().unwrap().to_owned().try_into().unwrap())
		.collect();
	let hours_selected = 24;
	let (normalized_df, dt_index) = collect_data(symbols, "5m".into(), hours_selected).await?;
	Ok(plotly_closes(normalized_df, dt_index))
}

pub async fn collect_data(symbols: Vec<Pair>, timeframe: Timeframe, hours_selected: i64) -> Result<(HashMap<Pair, Vec<f64>>, Vec<DateTime<Utc>>)> {
	//HACK: assumes we're never misaligned here,
	assert!(hours_selected <= 24, "Should have an Error for this but I can't be bothered");
	let data: Arc<Mutex<HashMap<Pair, Vec<f64>>>> = Arc::new(Mutex::new(HashMap::new()));
	let dt_index: Arc<Mutex<Vec<DateTime<Utc>>>> = Arc::new(Mutex::new(Vec::new()));
	let fetch_tasks = symbols.iter().map(|symbol| {
		let data = Arc::clone(&data);
		let dt_index = Arc::clone(&dt_index);

		tokio::spawn(async move {
			match get_historical_data(symbol.clone(), timeframe, hours_selected).await {
				Ok(series) => {
					let mut data = data.lock().unwrap();
					data.insert(*symbol, series.col_closes);
					if &symbol.to_string() == "BTCUSDT" {
						*dt_index.lock().unwrap() = series.col_open_times;
					}
				}
				Err(e) => {
					eprintln!("Failed to fetch data for symbol: {}. Error: {}", symbol, e);
				}
			}
		})
	});
	join_all(fetch_tasks).await;

	let data: HashMap<Pair, Vec<f64>> = Arc::try_unwrap(data).unwrap().into_inner().unwrap();
	let dt_index = Arc::try_unwrap(dt_index).unwrap().into_inner().unwrap();
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
pub async fn get_historical_data(symbol: Pair, timeframe: Timeframe, hours_selected: i64) -> Result<RelevantHistoricalData> {
	let time_ago: DateTime<Utc> = Utc::now() - Duration::hours(hours_selected);
	let time_ago_ms = time_ago.timestamp_millis();
	let url = format!("https://api.binance.com/api/v3/klines?symbol={symbol}&interval={timeframe}&startTime={time_ago_ms}");
	let client = reqwest::Client::new();
	let response = client.get(&url).send().await?;

	#[serde_as]
	#[allow(unused)]
	#[derive(Clone, Debug, Default, Deserialize)]
	struct Kline {
		open_time: i64,
		#[serde_as(as = "DisplayFromStr")]
		open: f64,
		#[serde_as(as = "DisplayFromStr")]
		high: f64,
		#[serde_as(as = "DisplayFromStr")]
		low: f64,
		#[serde_as(as = "DisplayFromStr")]
		close: f64,
		#[serde_as(as = "DisplayFromStr")]
		volume: f64,
		close_time: i64,
		#[serde_as(as = "DisplayFromStr")]
		quote_asset_volume: f64,
		trades: u32,
		#[serde_as(as = "DisplayFromStr")]
		taker_buy_base: f64,
		#[serde_as(as = "DisplayFromStr")]
		taker_by_quote: f64,
		ignore: String,
	}
	let raw_data: Vec<Kline> = deser_reqwest(response).await?;

	let mut open_time = Vec::new();
	let mut open = Vec::new();
	let mut high = Vec::new();
	let mut low = Vec::new();
	let mut close = Vec::new();
	let mut volume = Vec::new();
	for kline in raw_data {
		open_time.push(DateTime::from_timestamp_millis(kline.open_time).unwrap());
		open.push(kline.open);
		high.push(kline.high);
		low.push(kline.low);
		close.push(kline.close);
		volume.push(kline.volume);
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

pub fn plotly_closes(normalized_closes: HashMap<Pair, Vec<f64>>, dt_index: Vec<DateTime<Utc>>) -> Plot {
	let mut performance: Vec<(Pair, f64)> = normalized_closes.iter().map(|(k, v)| (k, (v[v.len() - 1] - v[0]))).collect();
	performance.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

	let n_samples = (performance.len() as f64).ln().round() as usize;
	let top: Vec<Pair> = performance.iter().rev().take(n_samples).map(|x| x.0.clone()).collect();
	let bottom: Vec<Pair> = performance.iter().take(n_samples).map(|x| x.0.clone()).collect();

	let mut plot = Plot::new();

	let mut add_trace = |name: Pair, width: f64, color: Option<&'static str>, legend: Option<String>| {
		let y_values: Vec<f64> = normalized_closes.get(&name).unwrap().to_owned();
		let x_values: Vec<usize> = (0..y_values.len()).collect();

		let mut line = Line::new().width(width);
		if let Some(c) = color {
			line = line.color(c);
		}

		let mut trace = Scatter::new(x_values, y_values).mode(plotly::common::Mode::Lines).line(line);
		if let Some(l) = legend {
			trace = trace.name(l.to_string());
		} else {
			trace = trace.show_legend(false);
		}
		plot.add_trace(trace);
	};

	let mut contains_btcusdt = false;
	for col_name in normalized_closes.keys() {
		if col_name == "BTCUSDT" {
			contains_btcusdt = true;
			continue;
		}
		if top.contains(col_name) || bottom.contains(col_name) {
			continue;
		}
		add_trace(col_name, 1.0, Some("grey"), None);
	}
	let mut labeled_trace = |col_name: &Pair, symbol: Option<&str>, line_width: f64, color: Option<&str>| {
		let p: f64 = performance.iter().find(|a| &a.0 == col_name).unwrap().1;
		let mut symbol = match symbol {
			Some(s) => s,
			None => &col_name[0..col_name.len() - 4].to_string().replace("1000", ""),
		};
		let sign = if p >= 0.0 { '+' } else { '-' };
		let change = format!("{:.2}", 100.0 * p.abs());
		let legend = format!("{:<5}{}{:>5}%", symbol, sign, change);
		add_trace(col_name, line_width, color, Some(legend));
	};
	for col_name in top.iter() {
		labeled_trace(col_name, None, 2.0, None);
	}
	if contains_btcusdt {
		labeled_trace("BTCUSDT".try_into().unwrap(), Some("~BTC~"), 3.5, Some("gold"));
	}
	for col_name in bottom.iter().rev() {
		labeled_trace(col_name, None, 2.0, None);
	}

	plot
}
