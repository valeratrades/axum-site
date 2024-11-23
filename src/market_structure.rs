use std::{collections::HashMap, error::Error, mem::transmute, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use color_eyre::eyre::Result;
use futures::future::join_all;
use plotly::{common::Line, Plot, Scatter};
use polars::prelude::*;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::Mutex;
use v_utils::{trades::Timeframe, utils::Df};
use crate::utils::deser_reqwest;

pub async fn run() -> Result<()> {
	let symbols = ["BTCUSDT", "ETHUSDT", "ADAUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT"]; //dbg
	let hours_selected = 24;
	println!("will request soon");
	let dbg_data = get_historical_data(symbols[0], "5m".into(), hours_selected).await?;
	dbg!(&dbg_data.col_closes[0]);
	//let normalized_df = collect_data(symbols, timeframe, hours_selected).await?;
	//plotly_closes(closes_df);
	Ok(())
}

//pub async fn collect_data(symbols: &[str], timeframe: u32, hours_selected: i64) -> Result<DataFrame, Box<dyn Error>> {
//	let data: Arc<Mutex<HashMap<String, DataFrame>>> = Arc::new(Mutex::new(HashMap::new()));
//	let fetch_tasks = symbols.iter().map(|symbol| {
//		let symbol = symbol.clone();
//		let data = Arc::clone(&data);
//
//		tokio::spawn(async move {
//			match get_historical_data(&symbol, timeframe, hours_selected).await {
//				Ok(df) => {
//					let mut data = data.lock().await;
//					data.insert(symbol.clone(), df);
//				}
//				Err(e) => {
//					eprintln!("Failed to fetch data for symbol: {}. Error: {}", symbol, e);
//				}
//			}
//		})
//	});
//	join_all(fetch_tasks).await;
//
//	let data = data.lock().await;
//	let mut normalized_series = Vec::new();
//	for (symbol, df) in data.iter() {
//		let close_series = df.column("close")?;
//		let first_close = close_series.get(0).unwrap_or(polars::prelude::AnyValue::Float64(1.0));
//
//		let normalized = close_series
//			.cast(&DataType::Float64)?
//			.f64()?
//			.into_iter()
//			.map(|opt_val| opt_val.map(|val| (val / first_close).ln()))
//			.collect::<Float64Chunked>();
//
//		normalized_series.push(Series::new(symbol.into(), normalized));
//	}
//
//	let normalized_df = DataFrame::new(normalized_series)?;
//	Ok(normalized_df)
//}

#[derive(Clone, Debug, Default, derive_new::new)]
struct RelevantHistoricalData {
	col_open_times: Vec<DateTime<Utc>>,
	col_opens: Vec<f64>,
	col_highs: Vec<f64>,
	col_lows: Vec<f64>,
	col_closes: Vec<f64>,
	col_volumes: Vec<f64>,
}
pub async fn get_historical_data(symbol: &str, timeframe: Timeframe, hours_selected: i64) -> Result<RelevantHistoricalData> {
	let time_ago: DateTime<Utc> = Utc::now() - Duration::hours(hours_selected);
	let time_ago_ms = time_ago.timestamp_millis();
	println!("{timeframe}");
	let url = format!("https://api.binance.com/api/v3/klines?symbol={symbol}&interval={timeframe}&startTime={time_ago_ms}");
	let client = reqwest::Client::new();
	let response = client.get(&url).send().await?;

	#[derive(Clone, Debug, Default, derive_new::new, Deserialize)]
	struct Kline {
		open_time: i64,
		open: f64,
		high: f64,
		low: f64,
		close: f64,
		volume: f64,
		close_time: i64,
		quote_asset_volume: f64,
		trades: u32,
		taker_buy_base: f64,
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

#[derive(Clone, Debug, Default, derive_new::new)]
struct NormCloses {
	symbol: String,
	values: Vec<f64>,
}
impl NormCloses {
	pub fn build(closes: Vec<f64>, symbol: String) -> Self {
		let first_close = closes[0];
		let values = closes.iter().map(|&close| (close / first_close).ln()).collect();
		Self { symbol, values }
	}
}

// problem is: it is a df, not a hashmap, the alignment of columns should be a property, not a promise
//fn plotly_closes(normalized_closes: &[(String, Vec<f64>)], shared_dt_index: Vec<DateTime<Utc>>) {
//	tuples.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
//	let ascending_performance = normalized_closes.sort_by(|a, b| (a.1[0] - a.1[len(a.1)]);
//	let log_size = (tuples.len() as f64).ln().round() as usize;
//	let top: Vec<String> = tuples.iter().rev().take(log_size).map(|x| x.0.clone()).collect();
//	let bottom: Vec<String> = tuples.iter().take(log_size).map(|x| x.0.clone()).collect();
//
//	let mut plot = Plot::new();
//
//	let mut add_trace = |name: &str, width: f64, color: Option<&str>, legend: Option<String>| {
//		let color_static: Option<&'static str> = color.map(|c| unsafe { transmute::<&str, &'static str>(c) });
//		let polars_vec: Vec<Option<f64>> = normalized_closes_df.column(name).unwrap().f64().unwrap().to_vec();
//		let y_values: Vec<f64> = polars_vec.iter().filter_map(|&x| x).collect();
//		let x_values: Vec<usize> = (0..y_values.len()).collect();
//
//		let mut line = Line::new().width(width);
//		if let Some(c) = color_static {
//			line = line.color(c);
//		}
//
//		let mut trace = Scatter::new(x_values, y_values).mode(plotly::common::Mode::Lines).line(line);
//		if let Some(l) = legend {
//			trace = trace.name(&l);
//		} else {
//			trace = trace.show_legend(false);
//		}
//		plot.add_trace(trace);
//	};
//
//	let mut contains_btcusdt = false;
//	for col_name in normalized_closes_df.get_column_names() {
//		if col_name == "BTCUSDT" {
//			contains_btcusdt = true;
//			continue;
//		}
//		if top.contains(&col_name.to_string()) || bottom.contains(&col_name.to_string()) {
//			continue;
//		}
//		add_trace(col_name, 1.0, Some("grey"), None);
//	}
//	for col_name in top.iter() {
//		let p: f64 = performance.column(col_name).unwrap().get(0).unwrap().try_extract().unwrap();
//		let mut symbol = col_name[0..col_name.len() - 4].to_string();
//		symbol = symbol.replace("1000", "");
//		let sign = if p >= 0.0 { '+' } else { '-' };
//		let change = format!("{:.2}", 100.0 * p.abs());
//		let legend = format!("{:<5}{}{:>5}%", symbol, sign, change);
//		add_trace(col_name, 2.0, None, Some(legend));
//	}
//	if contains_btcusdt {
//		let p: f64 = performance.column("BTCUSDT").unwrap().get(0).unwrap().try_extract().unwrap();
//		add_trace("BTCUSDT", 3.5, Some("gold"), Some(format!("~BTC~ {:>5}", format!("{:.2}", 100.0 * p))));
//	}
//	for col_name in bottom.iter().rev() {
//		let p: f64 = performance.column(col_name).unwrap().get(0).unwrap().try_extract().unwrap();
//		let mut symbol = col_name[0..col_name.len() - 4].to_string();
//		symbol = symbol.replace("1000", "");
//		let sign = if p >= 0.0 { '+' } else { '-' };
//		let change = format!("{:.2}", 100.0 * p.abs());
//		let legend = format!("{:<5}{}{:>5}%", symbol, sign, change);
//		add_trace(col_name, 2.0, None, Some(legend));
//	}
//
//	plot.show();
//}
