use v_exchanges::{binance, prelude::*};

#[tokio::main]
async fn main() {
	color_eyre::install().unwrap();
	v_utils::utils::init_subscriber(v_utils::utils::LogDestination::xdg(env!("CARGO_PKG_NAME")));

	let tf = "5m".into();
	let range = (24 * 12 + 1).into();

	let bn = binance::Binance::default();
	let m = "Binance/Futures".into();
	let pairs = bn.exchange_info(m).await.unwrap().usdt_pairs().collect::<Vec<_>>();

	for p in pairs {
		let lsr = bn.lsr(p, tf, range, "Global".into()).await.unwrap();
		if lsr.len() == 0 {
			tracing::info!("No data for {}", p);
			continue;
		}
		let diff = lsr[0].long() - lsr[lsr.len() - 1].long();
		dbg!(&diff);
	}
}
