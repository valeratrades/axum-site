use std::sync::{Arc, RwLock};

use axum::{Router, extract::State, response::Html, routing::get};
use tokio::net::TcpListener;
use v_exchanges::RequestRange;
use v_utils::trades::Timeframe;

mod lsr;
mod market_structure;

struct AppState {
	plot_html: String,
	lsr_str: String,
}

//NB: all axum handlers are expected to be async
#[tokio::main]
async fn main() {
	color_eyre::install().unwrap();

	let state = Arc::new(RwLock::new(AppState {
		plot_html: String::new(),
		lsr_str: String::new(),
	}));

	let tf = "5m".into();
	let range = (24 * 12 + 1).into(); //24h, given `5m` tf
	let state_clone = state.clone();
	tokio::spawn(async move {
		update_plot(range, tf, state_clone).await;
	});

	let lsr_str = lsr::get(tf, range).await;
	if let Ok(mut state) = state.write() {
		state.lsr_str = lsr_str;
	}

	let app = Router::new().route("/", get(handler)).with_state(state);

	let listener = TcpListener::bind("127.0.0.1:53863").await.unwrap();
	println!("listening on {}", listener.local_addr().unwrap());
	axum::serve(listener, app).await.unwrap();
}

async fn handler(State(state): State<Arc<RwLock<AppState>>>) -> Html<String> {
	let state = state.read().unwrap();
	let html = state.plot_html.clone();
	Html(format!(
		"{}\n<div style='width: 1600px; height: 400px; margin: 20px auto'><pre style='margin: 0; height: 100%; overflow: auto'>{}</pre></div>",
		html, state.lsr_str
	))
}
async fn update_plot(limit: RequestRange, tf: Timeframe, state: Arc<RwLock<AppState>>) {
	let m: v_exchanges::AbsMarket = "Binance/Futures".into();

	loop {
		match market_structure::try_build(limit, tf, m).await {
			Ok(new_plot) => {
				let mut state = state.write().unwrap();
				state.plot_html = new_plot.to_html();
			}
			Err(err) => {
				eprintln!("Failed to update plot: {}", err);
			}
		}
		tokio::time::sleep(tokio::time::Duration::from_secs(60 * 60)).await;
	}
}
