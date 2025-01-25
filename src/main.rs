#![feature(duration_constructors)]
use std::sync::{Arc, RwLock};

use axum::{Router, extract::State, response::Html, routing::get};
use tokio::net::TcpListener;
use v_exchanges::RequestRange;
use v_utils::prelude::*;

mod cme;
mod lsr;
mod market_structure;

struct AppState {
	plot_html: String,
	lsr_str: String,
	cme_str: String,
}

#[tokio::main]
async fn main() {
	clientside!();

	let state = Arc::new(RwLock::new(AppState {
		plot_html: "Waiting for MarketStructure data...".into(),
		lsr_str: "Waiting for LSR data...".into(),
		cme_str: "Waiting for CME data...".into(),
	}));

	let tf = "5m".into();
	let range = (24 * 12 + 1).into(); // 24h, given `5m` tf
	let state_clone = state.clone();
	tokio::spawn(async move {
		update_plot(range, tf, state_clone).await;
	});

	let lsr_str = lsr::get(tf, range).await;
	if let Ok(mut state) = state.write() {
		state.lsr_str = lsr_str.unwrap_or_else(|e| format!("Failed to fetch LSR data: {}", e));
	}

	let cme_str = cme::fetch_cftc_positions().await;
	if let Ok(mut state) = state.write() {
		state.cme_str = cme_str.unwrap_or_else(|e| format!("Failed to fetch CME data: {}", e));
	}

	let app = Router::new().route("/", get(handler)).with_state(state);

	let listener = TcpListener::bind("127.0.0.1:53863").await.unwrap();
	println!("listening on {}", listener.local_addr().unwrap());
	axum::serve(listener, app).await.unwrap();
}

async fn handler(State(state): State<Arc<RwLock<AppState>>>) -> Html<String> {
	let state = state.read().unwrap();
	std::fs::write("./tmp/plot.html", &state.plot_html).unwrap();
	let html = state.plot_html.clone();
	Html(format!(
		r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Resizable Boxes</title>
            <style>
                body {{
                    margin: 0;
                    padding: 20px;
                    box-sizing: border-box;
                    height: 100vh;
                    display: flex;
                    flex-direction: column;
                    gap: 20px;
                }}
                .container {{
                    display: flex;
                    gap: 20px;
                    flex: 1;
                    min-height: 0;
                }}
                .resizable {{
                    width: fit-content;
                    height: fit-content;
                    max-width: 100%;
                    max-height: 100%;
                    overflow: auto;
                    border: 1px solid #ccc;
                    position: relative;
                }}
                .resizable .resizer {{
                    width: 10px;
                    height: 10px;
                    background: #ccc;
                    position: absolute;
                    right: 0;
                    bottom: 0;
                    cursor: se-resize;
                }}
            </style>
        </head>
        <body>
            {}
            <div class="container">
                <div class="resizable">
                    <pre style="margin: 0;">{}</pre>
                    <div class="resizer"></div>
                </div>
                <div class="resizable">
                    <pre style="margin: 0;">{}</pre>
                    <div class="resizer"></div>
                </div>
            </div>
            <script>
                document.querySelectorAll('.resizer').forEach(resizer => {{
                    const resizable = resizer.parentElement;
                    let startX, startY, startWidth, startHeight;

                    resizer.addEventListener('mousedown', initDrag, false);

                    function initDrag(e) {{
                        startX = e.clientX;
                        startY = e.clientY;
                        startWidth = parseInt(document.defaultView.getComputedStyle(resizable).width, 10);
                        startHeight = parseInt(document.defaultView.getComputedStyle(resizable).height, 10);
                        document.documentElement.addEventListener('mousemove', doDrag, false);
                        document.documentElement.addEventListener('mouseup', stopDrag, false);
                    }}

                    function doDrag(e) {{
                        const newWidth = startWidth + e.clientX - startX;
                        const newHeight = startHeight + e.clientY - startY;
                        const maxWidth = window.innerWidth - resizable.offsetLeft - 20; // 20px padding
                        const maxHeight = window.innerHeight - resizable.offsetTop - 20; // 20px padding

                        resizable.style.width = Math.min(newWidth, maxWidth) + 'px';
                        resizable.style.height = Math.min(newHeight, maxHeight) + 'px';
                    }}

                    function stopDrag() {{
                        document.documentElement.removeEventListener('mousemove', doDrag, false);
                        document.documentElement.removeEventListener('mouseup', stopDrag, false);
                    }}
                }});
            </script>
        </body>
        </html>
        "#,
		html, state.lsr_str, state.cme_str
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
