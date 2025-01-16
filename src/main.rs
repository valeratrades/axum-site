use std::sync::{Arc, RwLock};

use axum::{Router, extract::State, response::Html, routing::get};
use tokio::net::TcpListener;

mod market_structure;
mod lsr;

//NB: all axum handlers are expected to be async
#[tokio::main]
async fn main() {
	color_eyre::install().unwrap();

	let plot_html = Arc::new(RwLock::new(String::new()));
	let plot_html_clone = plot_html.clone();
	tokio::spawn(async move {
		update_plot(plot_html_clone).await;
	});

	let app = Router::new().route("/", get(handler)).with_state(plot_html);

	let tf = "5m".into();
	let range = (24 * 12 + 1).into();
	let lsr_str = lsr::get(tf, range).await;
	//TODO!!!!!!!: display on the site
	dbg!(&lsr_str);

	let listener = TcpListener::bind("127.0.0.1:53863").await.unwrap();
	println!("listening on {}", listener.local_addr().unwrap());
	axum::serve(listener, app).await.unwrap();
}

async fn handler(State(plot_html): State<Arc<RwLock<String>>>) -> Html<String> {
	let plot_html = plot_html.read().unwrap(); // Read the current plot
	Html(plot_html.clone())
}

async fn update_plot(plot_html: Arc<RwLock<String>>) {
	let m: v_exchanges::AbsMarket = "Binance/Futures".into();
	let hours_back = 24;
	let tf = "5m".into();

	loop {
		match market_structure::try_build(hours_back, tf, m).await {
			Ok(new_plot) => {
				let mut html_guard = plot_html.write().unwrap();
				*html_guard = new_plot.to_html();
			}
			Err(err) => {
				eprintln!("Failed to update plot: {}", err);
			}
		}
		tokio::time::sleep(tokio::time::Duration::from_secs(60 * 60)).await;
	}
}
