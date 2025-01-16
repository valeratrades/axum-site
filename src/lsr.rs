use futures::future::join_all;
use v_exchanges::{
	adapters::{binance::BinanceOption, v_exchanges_api_generics::http::RequestConfig},
	binance::{self, data::Lsrs},
	prelude::*,
};
use v_utils::{NowThen, trades::Timeframe};

const SLICE_SIZE: usize = 10;

pub async fn get(tf: Timeframe, range: RequestRange) -> String {
	let mut bn = binance::Binance::default();
	let mut request_conf = RequestConfig::default();
	request_conf.timeout = std::time::Duration::from_secs(10); // default `recvWindow`, (how Binance calls it), for futs is 5000ms (2024/01/16)
	bn.update_default_option(BinanceOption::RequestConfig(request_conf));

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
					//XXX: Many just time out, need to tweak response timeout
					tracing::warn!("Couldn't fetch data for {}: {:?}", p, e);
					None
				}
			}
		}
	});
	let results = join_all(handles).await;

	let mut lsrs = results.into_iter().flatten().collect::<Vec<_>>();
	lsrs.sort_by(|a, b| a.last().unwrap().long().partial_cmp(&b.last().unwrap().long()).unwrap());

	let mut s = String::new();
	let display_rows_ceiling = std::cmp::min(SLICE_SIZE, lsrs.len() / 2 /*floor*/);
	for i in 0..display_rows_ceiling {
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
