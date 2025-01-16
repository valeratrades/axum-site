use futures::future::join_all;
use v_exchanges::{
	binance::{self, data::Lsrs},
	prelude::*,
};
use v_utils::{NowThen, trades::Timeframe};

const SLICE_SIZE: usize = 10;

pub async fn get(tf: Timeframe, range: RequestRange) -> String {
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

	let mut s = String::new();
	for i in 0..SLICE_SIZE {
		let (short_outlier, long_outlier) = (&lsrs[i], &lsrs[lsrs.len() - i - 1]);
		s.push_str(format!("{}{}", fmt_lsr(short_outlier), fmt_lsr(long_outlier)).as_str());
	}
	s
}

fn fmt_lsr(lsrs: &Lsrs) -> String {
	let diff = NowThen::new(*lsrs.first().unwrap().long, *lsrs.first().unwrap().long);
	let diff_f = format!("{diff}%");
	format!("  ├{:<9}: {diff_f:<8}", &lsrs.pair.base().to_string()) // `to_string`s are required because rust is dumb as of today (2024/01/16)
}
