use futures::future::join_all;
use v_exchanges::{binance, prelude::*};

const SLICE_SIZE: usize = 10;

#[tokio::main]
async fn main() {
	v_utils::clientside!();

	let tf = "5m".into();
	let range = (24 * 12 + 1).into();

	let bn = binance::Binance::default();
	let m = "Binance/Futures".into();
	let pairs = bn.exchange_info(m).await.unwrap().usdt_pairs().collect::<Vec<_>>();

	let handles = pairs.iter().map(|p| {
		let bn = bn.clone();
		async move {
			match bn.lsr(*p, tf, range, "Global".into()).await {
				Ok(lsr_vec) if !lsr_vec.is_empty() => Some(lsr_vec),
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
	let results = join_all(handles).await;

	let mut lsrs = results.into_iter().flatten().collect::<Vec<_>>();
	lsrs.sort_by(|a, b| a.last().unwrap().long().partial_cmp(&b.last().unwrap().long()).unwrap());

	for i in 0..SLICE_SIZE {
		let (short_outlier, long_outlier) = (&lsrs[i], &lsrs[lsrs.len() - i - 1]);

		//let diff = lsr[0].long() - lsr[lsr.len() - 1].long();
		//println!("{:?} - {:?}", lsr[0].time(), diff);
		println!(
			"{}: {}.....{}: {}",
			short_outlier.pair,
			short_outlier.last().unwrap().long,
			long_outlier.pair,
			long_outlier.last().unwrap().long
		);
	}

	//let diff = lsr[0].long() - lsr[lsr.len() - 1].long();
}
