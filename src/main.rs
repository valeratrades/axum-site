use std::sync::{Arc, RwLock};

use axum::{Router, extract::State, response::Html, routing::get};
use tokio::net::TcpListener;

mod market_structure;
use v_utils::io::ExpandedPath;

//NB: all axum handlers are expected to be async
#[tokio::main]
async fn main() {
	color_eyre::install().unwrap();

	let pairs_file = ExpandedPath::from(std::env::args().nth(1).unwrap());

	let plot_html = Arc::new(RwLock::new(String::new()));

	// Start the plot updating task
	let pairs_file_clone = pairs_file.clone();
	let plot_html_clone = plot_html.clone();
	tokio::spawn(async move {
		update_plot(plot_html_clone, pairs_file_clone).await;
	});

	let app = Router::new().route("/", get(handler)).with_state(plot_html);

	let listener = TcpListener::bind("127.0.0.1:53863").await.unwrap();
	println!("listening on {}", listener.local_addr().unwrap());
	axum::serve(listener, app).await.unwrap();
}

async fn handler(State(plot_html): State<Arc<RwLock<String>>>) -> Html<String> {
	let plot_html = plot_html.read().unwrap(); // Read the current plot
	Html(plot_html.clone())
}

async fn update_plot(plot_html: Arc<RwLock<String>>, pairs_file: ExpandedPath) {
	loop {
		match market_structure::try_build(&pairs_file).await {
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
