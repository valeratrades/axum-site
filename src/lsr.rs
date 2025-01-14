use futures::future::join_all;
use v_exchanges::{binance, prelude::*};

#[tokio::main]
async fn main() {
	v_utils::clientside!();

	let tf = "5m".into();
	let range = (24 * 12 + 1).into();

	let bn = binance::Binance::default();
	let m = "Binance/Futures".into();
	let pairs = bn.exchange_info(m).await.unwrap().usdt_pairs().collect::<Vec<_>>();

	let futures = pairs.iter().map(|p| {
		let bn = bn.clone();
		async move {
			match bn.lsr(*p, tf, range, "Global".into()).await {
				Ok(lsr) if !lsr.is_empty() => Some((p, lsr[0].long() - lsr[lsr.len() - 1].long())),
				Ok(_) => {
					tracing::info!("No data for {}", p);
					None
				}
				Err(e) => {
					tracing::error!("Error fetching data for {}: {:?}", p, e);
					None
				}
			}
		}
	});
	let results = join_all(futures).await;

	for r in results.into_iter().flatten() {
		let (pair, diff) = r;
		dbg!(&pair, &diff);
	}
}

