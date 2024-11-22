use std::{collections::HashMap, error::Error, mem::transmute, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use futures::future::join_all;
use plotly::{common::Line, Plot, Scatter};
use polars::prelude::*;
use serde_json::Value;
use tokio::sync::Mutex;

pub async fn run() {
	let symbols = ["BTCUSDT", "ETHUSDT", "ADAUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT"]; //dbg
	let normalized_df = collect_data(symbols, timeframe, hours_selected).await?;
	plotly_closes(closes_df);
}

pub async fn collect_data(symbols: Vec<String>, timeframe: u32, hours_selected: i64) -> Result<DataFrame, Box<dyn Error>> {
	// Create shared data storage
	let data: Arc<Mutex<HashMap<String, DataFrame>>> = Arc::new(Mutex::new(HashMap::new()));

	// Create fetch tasks for each symbol
	let fetch_tasks = symbols.iter().map(|symbol| {
		let symbol = symbol.clone();
		let data = Arc::clone(&data);

		tokio::spawn(async move {
			match get_historical_data(&symbol, timeframe, hours_selected).await {
				Ok(df) => {
					let mut data = data.lock().await;
					data.insert(symbol.clone(), df);
				}
				Err(e) => {
					eprintln!("Failed to fetch data for symbol: {}. Error: {}", symbol, e);
				}
			}
		})
	});

	// Wait for all fetch tasks to complete
	join_all(fetch_tasks).await;

	// Get the locked data
	let data = data.lock().await;

	// Create normalized DataFrame
	let mut normalized_series = Vec::new();

	for (symbol, df) in data.iter() {
		let close_series = df.column("close")?;
		let first_close = close_series.get(0).unwrap_or(1.0);

		// Normalize and take log
		let normalized = close_series
			.cast(&DataType::Float64)?
			.f64()?
			.into_iter()
			.map(|opt_val| opt_val.map(|val| (val / first_close).ln()))
			.collect::<Float64Chunked>();

		normalized_series.push(Series::new(symbol, normalized));
	}

	// Create final DataFrame
	let normalized_df = DataFrame::new(normalized_series)?;
	Ok(normalized_df)
}

pub async fn get_historical_data(symbol: &str, timeframe: u32, hours_selected: i64) -> Result<DataFrame, Box<dyn Error>> {
	// Calculate start time
	let time_ago: DateTime<Utc> = Utc::now() - Duration::hours(hours_selected);
	let time_ago_ms = time_ago.timestamp_millis();

	// Construct URL
	let url = format!("https://api.binance.com/api/v3/klines?symbol={}&interval={}m&startTime={}", symbol, timeframe, time_ago_ms);

	// Fetch data
	let client = reqwest::Client::new();
	let response = client.get(&url).send().await?;
	let raw_data: Vec<Value> = response.json().await?;

	// Convert raw data to vectors
	let mut open_time = Vec::new();
	let mut open = Vec::new();
	let mut high = Vec::new();
	let mut low = Vec::new();
	let mut close = Vec::new();
	let mut volume = Vec::new();
	let mut close_time = Vec::new();
	let mut quote_asset_volume = Vec::new();
	let mut trades = Vec::new();
	let mut taker_buy_base = Vec::new();
	let mut taker_buy_quote = Vec::new();
	let mut ignore = Vec::new();

	for item in raw_data {
		if let Value::Array(arr) = item {
			open_time.push(arr[0].as_i64().unwrap_or_default());
			open.push(arr[1].as_str().unwrap_or_default().parse::<f64>().unwrap_or_default());
			high.push(arr[2].as_str().unwrap_or_default().parse::<f64>().unwrap_or_default());
			low.push(arr[3].as_str().unwrap_or_default().parse::<f64>().unwrap_or_default());
			close.push(arr[4].as_str().unwrap_or_default().parse::<f64>().unwrap_or_default());
			volume.push(arr[5].as_str().unwrap_or_default().parse::<f64>().unwrap_or_default());
			close_time.push(arr[6].as_i64().unwrap_or_default());
			quote_asset_volume.push(arr[7].as_str().unwrap_or_default().parse::<f64>().unwrap_or_default());
			trades.push(arr[8].as_i64().unwrap_or_default());
			taker_buy_base.push(arr[9].as_str().unwrap_or_default().parse::<f64>().unwrap_or_default());
			taker_buy_quote.push(arr[10].as_str().unwrap_or_default().parse::<f64>().unwrap_or_default());
			ignore.push(arr[11].as_str().unwrap_or_default().parse::<f64>().unwrap_or_default());
		}
	}

	// Create DataFrame
	let mut df = DataFrame::new(vec![
		Series::new("open_time", open_time),
		Series::new("open", open),
		Series::new("high", high),
		Series::new("low", low),
		Series::new("close", close),
		Series::new("volume", volume),
		Series::new("close_time", close_time),
		Series::new("quote_asset_volume", quote_asset_volume),
		Series::new("trades", trades),
		Series::new("taker_buy_base", taker_buy_base),
		Series::new("taker_buy_quote", taker_buy_quote),
		Series::new("ignore", ignore),
	])?;

	// Convert timestamp to datetime
	df = df
		.lazy()
		.with_column(col("open_time").cast(DataType::Datetime(TimeUnit::Milliseconds, None)).alias("open_time"))
		.collect()?;

	// Calculate returns and cumulative returns
	let close_series = df.column("close")?;
	let returns = close_series
		.shift(1)
		.map(|opt_prev| opt_prev.map(|prev| close_series.iter().map(|curr| curr.unwrap_or(0.0) / prev - 1.0 + 1.0).collect::<Float64Chunked>()))
		.unwrap();

	// Set first return to 1.0
	let mut returns = returns.to_owned();
	returns.set(0, Some(1.0))?;

	// Calculate cumulative returns
	let cum_returns = returns.cum_prod();

	// Calculate variance
	let variance = close_series.var(1);

	// Add new columns to DataFrame
	df.with_column(Series::new("return", returns))?;
	df.with_column(Series::new("cumulative_return", cum_returns))?;
	df.with_column(Series::new("variance", vec![variance; df.height()]))?;

	Ok(df)
}

//TODO!: transfer to use Prices from v_utils (or something like it, haven't fixed the name yet. Spinning up a df instance for just this seems wasteful)
fn plotly_closes(normalized_closes_df: DataFrame) {
	let performance = normalized_closes_df.tail(Some(1));
	let mut tuples: Vec<(String, f64)> = performance
		.get_column_names()
		.iter()
		.map(|&s| (s.to_owned(), performance.column(s).unwrap().get(0).unwrap().try_extract().unwrap()))
		.collect();
	tuples.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
	let log_size = (tuples.len() as f64).ln().round() as usize;
	let top: Vec<String> = tuples.iter().rev().take(log_size).map(|x| x.0.clone()).collect();
	let bottom: Vec<String> = tuples.iter().take(log_size).map(|x| x.0.clone()).collect();

	let mut plot = Plot::new();

	let mut add_trace = |name: &str, width: f64, color: Option<&str>, legend: Option<String>| {
		let color_static: Option<&'static str> = color.map(|c| unsafe { transmute::<&str, &'static str>(c) });
		let polars_vec: Vec<Option<f64>> = normalized_closes_df.column(name).unwrap().f64().unwrap().to_vec();
		let y_values: Vec<f64> = polars_vec.iter().filter_map(|&x| x).collect();
		let x_values: Vec<usize> = (0..y_values.len()).collect();

		let mut line = Line::new().width(width);
		if let Some(c) = color_static {
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
	for col_name in normalized_closes_df.get_column_names() {
		if col_name == "BTCUSDT" {
			contains_btcusdt = true;
			continue;
		}
		if top.contains(&col_name.to_string()) || bottom.contains(&col_name.to_string()) {
			continue;
		}
		add_trace(col_name, 1.0, Some("grey"), None);
	}
	for col_name in top.iter() {
		let p: f64 = performance.column(col_name).unwrap().get(0).unwrap().try_extract().unwrap();
		let mut symbol = col_name[0..col_name.len() - 4].to_string();
		symbol = symbol.replace("1000", "");
		let sign = if p >= 0.0 { '+' } else { '-' };
		let change = format!("{:.2}", 100.0 * p.abs());
		let legend = format!("{:<5}{}{:>5}%", symbol, sign, change);
		add_trace(col_name, 2.0, None, Some(legend));
	}
	if contains_btcusdt {
		let p: f64 = performance.column("BTCUSDT").unwrap().get(0).unwrap().try_extract().unwrap();
		add_trace("BTCUSDT", 3.5, Some("gold"), Some(format!("~BTC~ {:>5}", format!("{:.2}", 100.0 * p))));
	}
	for col_name in bottom.iter().rev() {
		let p: f64 = performance.column(col_name).unwrap().get(0).unwrap().try_extract().unwrap();
		let mut symbol = col_name[0..col_name.len() - 4].to_string();
		symbol = symbol.replace("1000", "");
		let sign = if p >= 0.0 { '+' } else { '-' };
		let change = format!("{:.2}", 100.0 * p.abs());
		let legend = format!("{:<5}{}{:>5}%", symbol, sign, change);
		add_trace(col_name, 2.0, None, Some(legend));
	}

	plot.show();
}
