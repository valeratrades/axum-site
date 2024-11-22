use axum::{response::Html, routing::get, Router};
mod market_structure;
use market_structure::run;

#[tokio::main]
async fn main() {
	run();
	let app = Router::new().route("/", get(handler));

	let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
	println!("listening on {}", listener.local_addr().unwrap());
	axum::serve(listener, app).await.unwrap();
}

async fn handler() -> Html<&'static str> {
	Html("<h1>Hello, Axum!</h1>")
}
