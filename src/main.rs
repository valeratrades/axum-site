#![feature(duration_constructors)]
use std::{
    fs,
    sync::{Arc, RwLock},
};

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::Html,
    routing::{get, post},
};
use clap::{Args, Parser, Subcommand};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use v_exchanges::RequestRange;
use v_utils::prelude::*;

mod cme;
mod lsr;
mod market_structure;

#[derive(Parser, Default)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long)]
    config: Option<ExpandedPath>,
}
#[derive(Subcommand)]
enum Commands {
    Serve(ServeArgs),
}
impl Default for Commands {
    fn default() -> Self {
        Commands::Serve(ServeArgs::default())
    }
}

#[derive(Debug, Args, Default)]
struct ServeArgs {
    #[arg(long)]
    mock: bool,
}

#[tokio::main]
async fn main() {
    clientside!();

    let cli = Cli::parse();
    match cli.command {
        Commands::Serve(args) => serve(args).await.unwrap(),
    }
}

async fn serve(args: ServeArgs) -> Result<()> {
    let dashboards = Arc::new(RwLock::new(DashboardsState {
        plot_html: "Waiting for MarketStructure data...".into(),
        lsr_str: "Waiting for LSR data...".into(),
        cme_str: "Waiting for CME data...".into(),
    }));
    let routes = vec![
        RouteInfo {
            path: "/".to_string(),
            description: "Home - List of all routes".to_string(),
            children: vec![],
        },
        RouteInfo {
            path: "/dashboards".to_string(),
            description: "Dashboards - Main dashboard view".to_string(),
            children: vec![],
        },
    ];
    let state = AppState::new(
        routes,
        Arc::clone(&dashboards),
    );

    match args.mock {
        true => {
            *dashboards.write().unwrap() = DashboardsState::load_mock()?;
        }
        false => {
            //TODO: get rid of await breaks, none of this should holt

            let tf = "5m".into();
            let range = (24 * 12 + 1).into(); // 24h, given `5m` tf
            let state_clone = dashboards.clone();
            tokio::spawn(async move {
                update_plot(range, tf, state_clone).await;
            });

            let lsr_str = lsr::get(tf, range).await;
            if let Ok(mut state) = dashboards.write() {
                state.lsr_str = lsr_str.unwrap_or_else(|e| format!("Failed to fetch LSR data: {}", e));
            }

            let cme_str = cme::fetch_cftc_positions().await;
            if let Ok(mut state) = dashboards.write() {
                state.cme_str = cme_str.unwrap_or_else(|e| format!("Failed to fetch CME data: {}", e));
            }
        }
    }

    let app = Router::new()
        .route("/", get(list_routes))
        .route("/dashboards", get(dashboards_handler))
        .route("/dashboards/snapshot", post(snapshot_handler))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:53863").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
#[derive(Clone, Debug, Serialize)]
struct RouteInfo {
    path: String,
    description: String,
    children: Vec<RouteInfo>,
}

async fn list_routes(State(state): State<AppState>) -> Html<String> {
    let routes = state.routes;
    let mut html = String::from("<ul>");
    for route in routes {
        html.push_str(&format!(
            r#"<li><a href="{}">{}</a> - {}"#,
            route.path, route.path, route.description
        ));
        if !route.children.is_empty() {
            html.push_str("<ul>");
            for child in route.children {
                html.push_str(&format!(
                    r#"<li><a href="{}">{}</a> - {}"#,
                    child.path, child.path, child.description
                ));
            }
            html.push_str("</ul>");
        }
        html.push_str("</li>");
    }
    html.push_str("</ul>");

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Route List</title>
        </head>
        <body>
            <h1>Available Routes</h1>
            {}
        </body>
        </html>
        "#,
        html
    ))
}

#[derive(Clone, Debug, derive_new::new)]
struct AppState {
    routes: Vec<RouteInfo>,
    dashboards: Arc<RwLock<DashboardsState>>,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
struct DashboardsState {
    plot_html: String,
    lsr_str: String,
    cme_str: String,
}
impl Mock for DashboardsState {
    const NAME: &'static str = "dashboards";
}

async fn dashboards_handler(State(state): State<AppState>) -> Html<String> {
    let state = state.dashboards.read().unwrap();
    std::fs::write("./tmp/plot.html", &state.plot_html).unwrap();
    let html = state.plot_html.clone();
    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <link rel="icon" type="image/jpg" href="/assets/me.jpg">
            <title>Dashboards</title>
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
                .notification {{
                    position: fixed;
                    top: 20px;
                    right: 20px;
                    padding: 10px 20px;
                    background-color: #4CAF50;
                    color: white;
                    border-radius: 5px;
                    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
                    display: none;
                    z-index: 1000;
                }}
            </style>
        </head>
        <body>
            <div id="notification" class="notification"></div>
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

                // Expose the snapshot function globally
                window.snapshot = function() {{
                    fetch('/dashboards/snapshot', {{ method: 'POST' }})
                        .then(response => {{
                            const notification = document.getElementById('notification');
                            if (response.ok) {{
                                notification.textContent = 'Snapshot saved successfully!';
                                notification.style.backgroundColor = '#4CAF50';
                            }} else {{
                                notification.textContent = 'Failed to save snapshot.';
                                notification.style.backgroundColor = '#f44336';
                            }}
                            notification.style.display = 'block';
                            setTimeout(() => {{
                                notification.style.display = 'none';
                            }}, 3000);
                        }});
                }};
            </script>
        </body>
        </html>
        "#,
        html, state.lsr_str, state.cme_str
    ))
}

async fn snapshot_handler(State(state): State<AppState>) -> StatusCode {
    let state = state.dashboards.read().unwrap();
    match state.persist() {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

//TODO; generalize to allow for specifiying all updates with given _frequency_ through this
async fn update_plot(limit: RequestRange, tf: Timeframe, state: Arc<RwLock<DashboardsState>>) {
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

pub trait Mock 
where Self: Sized + DeserializeOwned + Serialize,
{
    const NAME: &'static str;
    fn persist(&self) -> std::io::Result<()> {
        info!("Persisting current {}", Self::NAME);
        let json = serde_json::to_string_pretty(self)?;
        debug!(?json);
        fs::write(share_dir!().join(format!("{}.json", Self::NAME)), json)?;
        Ok(())
    }
    fn load_mock() -> std::io::Result<Self> {
        let json = fs::read_to_string(share_dir!().join(format!("{}.json", Self::NAME)))?;
        Ok(serde_json::from_str(&json)?)
    }
}
