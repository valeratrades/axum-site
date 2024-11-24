use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use chrono::{DateTime, Duration, Utc};
use color_eyre::eyre::Result;
use futures::future::join_all;
use plotly::{Plot, Scatter, common::Line};
use serde::Deserialize;
use serde_with::{DisplayFromStr, serde_as};
use v_utils::{trades::Timeframe, utils::Df};

use crate::utils::deser_reqwest;

pub async fn run() -> Result<()> {
	let symbols = ["BTCUSDT", "ETHUSDT", "ADAUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT"]; //dbg
	let symbols: Vec<String> = symbols.into_iter().map(|s| s.to_owned()).collect();
	let hours_selected = 24;
	let normalized_df = collect_data(&symbols, "5m".into(), hours_selected).await?;
	println!("{:?}", normalized_df);
	//plotly_closes(closes_df);
	Ok(())
}

pub async fn collect_data(symbols: &[String], timeframe: Timeframe, hours_selected: i64) -> Result<HashMap<String, Vec<f64>>> {
	//HACK: assumes we're never misaligned here,
	let data: Arc<Mutex<HashMap<String, Vec<f64>>>> = Arc::new(Mutex::new(HashMap::new()));
	let fetch_tasks = symbols.iter().map(|symbol| {
		let symbol = symbol.clone();
		let data = Arc::clone(&data);

		tokio::spawn(async move {
			match get_historical_data(&symbol, timeframe, hours_selected).await {
				Ok(series) => {
					let mut data = data.lock().unwrap();
					data.insert(symbol.clone(), series.col_closes);
				}
				Err(e) => {
					eprintln!("Failed to fetch data for symbol: {}. Error: {}", symbol, e);
				}
			}
		})
	});
	join_all(fetch_tasks).await;

	let data: HashMap<String, Vec<f64>> = Arc::try_unwrap(data).unwrap().into_inner().unwrap();
	let mut normalized_df: HashMap<String, Vec<f64>> = HashMap::new();
	for (symbol, closes) in data.into_iter() {
		let first_close = *closes.first().unwrap();
		let normalized = closes.into_iter().map(|p| (p / first_close).ln()).collect();
		normalized_df.insert(symbol, normalized);
	}
	Ok(normalized_df)
}

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

	#[serde_as]
	#[derive(Clone, Debug, Default, derive_new::new, Deserialize)]
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
