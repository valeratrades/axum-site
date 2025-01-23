use std::fmt::Write as _;

use futures::future::join_all;
use v_exchanges::{
	adapters::{binance::BinanceOption, v_exchanges_api_generics::http::RequestConfig},
	binance::{self, data::Lsrs},
	prelude::*,
};
use v_utils::prelude::*;

const SLICE_SIZE: usize = 10;

pub async fn get(tf: Timeframe, range: RequestRange) -> Result<String> {
	let mut bn = binance::Binance::default();
	let mut request_conf = RequestConfig::default();
	//TODO: switch to recvWinow. `timeout` is incorrect thing to use here.
	request_conf.timeout = std::time::Duration::from_secs(10);
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
		if i == 0 {
			let shorted_title = "Most Shorted";
			let longed_title = "Most Longed";
			s.write_fmt(format_args!("{:<26}{:<26}\n", shorted_title, longed_title)).unwrap(); // match formatting of `fmt_lsr` (when counting, don't forget all symbols outside of main paddings)
		} else {
			s.push('\n');
		}
		let (short_outlier, long_outlier) = (&lsrs[i], &lsrs[lsrs.len() - i - 1]);
		s.push_str(format!("{}{}", fmt_lsr(short_outlier), fmt_lsr(long_outlier)).as_str());
	}
	Ok(s)
}

fn fmt_lsr(lsrs: &Lsrs) -> String {
	let diff = NowThen::new(*lsrs.get(lsrs.len() - 1).unwrap().long, *lsrs.get(0).unwrap().long);
	let diff_f = format!("{diff}%");
	format!("  ├{:<9}: {:<12}", &lsrs.pair.base().to_string(), diff_f) // `to_string`s are required because rust is dumb as of today (2024/01/16)
}
