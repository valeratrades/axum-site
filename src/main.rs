use axum::{Router, response::Html, routing::get};
mod market_structure;
pub mod utils;

#[tokio::main]
async fn main() {
	color_eyre::install().unwrap();
	market_structure::run().await.unwrap();

	//let app = Router::new().route("/", get(handler));
	//
	//let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
	//println!("listening on {}", listener.local_addr().unwrap());
	//axum::serve(listener, app).await.unwrap();
}

async fn handler() -> Html<&'static str> {
	Html("<h1>Hello, Axum!</h1>")
}
