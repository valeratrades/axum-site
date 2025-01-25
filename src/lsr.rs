use futures::future::join_all;
use v_exchanges::{
	binance::{self, data::Lsrs},
	prelude::*,
};
use v_utils::prelude::*;

const SLICE_SIZE: usize = 10;

pub async fn get(tf: Timeframe, range: RequestRange) -> Result<String> {
	let mut bn = binance::Binance::default();
	bn.set_max_tries(3);

	let m = "Binance/Futures".into();
	let pairs = bn.exchange_info(m).await.unwrap().usdt_pairs().collect::<Vec<_>>();
	let pairs_len = pairs.len();

	let lsr_no_data_pairs_file = state_dir!().join("lsr_no_data_pairs");
	let lsr_no_data_pairs = match std::fs::metadata(&lsr_no_data_pairs_file) {
		Ok(metadata) => {
			let age = metadata.modified().unwrap().elapsed().unwrap();
			// Binance could start supporting any of the ignored pairs, so we refetch once a month
			if age < std::time::Duration::from_hours(30 * 24) {
				std::fs::read_to_string(&lsr_no_data_pairs_file)
					.map(|s| s.lines().filter(|s| !s.is_empty()).map(|s| s.into()).collect())
					.unwrap_or_else(|_| Vec::new())
			} else {
				Vec::new()
			}
		}
		Err(_) => Vec::new(),
	};
	let lsr_pairs = pairs.into_iter().filter(|p| !lsr_no_data_pairs.contains(&p.to_string())).collect::<Vec<_>>();

	let bn_arc = Arc::new(bn);
	let new_no_data_pairs = Arc::new(Mutex::new(Vec::new()));
	let handles = lsr_pairs.iter().map(|p| {
		let bn = Arc::clone(&bn_arc);
		let new_no_data_pairs = Arc::clone(&new_no_data_pairs);
		async move {
			match bn.lsr(*p, tf, range, "Global".into()).await {
				Ok(lsr_vec) if !lsr_vec.is_empty() => Some(lsr_vec),
				Ok(_) => {
					//TODO: write all pairs explicitly without data to XDG_STATE, retry for all once a month
					info!("No data for {}", p);
					new_no_data_pairs.lock().unwrap().push(p.to_string());
					None
				}
				Err(e) => {
					warn!("Couldn't fetch data for {}: {:?}", p, e);
					None
				}
			}
		}
	});
	let results = join_all(handles).await;
	let new_no_data_pairs = Arc::try_unwrap(new_no_data_pairs).expect("All locks have been awaited").into_inner().unwrap();
	if !new_no_data_pairs.is_empty() {
		let all_no_data_pairs = lsr_no_data_pairs.into_iter().chain(new_no_data_pairs).collect::<Vec<_>>();
		std::fs::write(&lsr_no_data_pairs_file, all_no_data_pairs.join("\n")).unwrap();
	}

	let mut lsrs = results.into_iter().flatten().collect::<Vec<_>>();
	lsrs.sort_by(|a, b| a.last().unwrap().long().partial_cmp(&b.last().unwrap().long()).unwrap());

	//TODO!!: show a) average b) number of pairs with collected data, against total
	let mut s = String::new();
	let display_rows_ceiling = std::cmp::min(SLICE_SIZE, lsrs.len() / 2 /*floor*/);
	for i in 0..display_rows_ceiling {
		if i == 0 {
			let shorted_title = "Most Shorted (% longs)";
			let longed_title = "Most Longed (% longs)";
			s.write_fmt(format_args!("{:<26}{:<26}\n", shorted_title, longed_title)).unwrap(); // match formatting of `fmt_lsr` (when counting, don't forget all symbols outside of main paddings)
		} else {
			s.push('\n');
		}
		let (short_outlier, long_outlier) = (&lsrs[i], &lsrs[lsrs.len() - i - 1]);
		s.push_str(format!("{}{}", fmt_lsr(short_outlier), fmt_lsr(long_outlier)).as_str());
	}
	s.push_str(&format!("\n{:-^26}", ""));
	s.push_str(&format!("\nAverage: {:.2}", lsrs.iter().map(|lsr| lsr.last().unwrap().long()).sum::<f64>() / lsrs.len() as f64));
	s.push_str(&format!("\nCollected for {}/{pairs_len} pairs on Binance", lsrs.len()));
	Ok(s)
}

fn fmt_lsr(lsrs: &Lsrs) -> String {
	let diff = NowThen::new(*lsrs.get(lsrs.len() - 1).unwrap().long, *lsrs.get(0).unwrap().long);
	let diff_f = format!("{diff}");
	format!("  ├{:<9}: {:<12}", &lsrs.pair.base().to_string(), diff_f) // `to_string`s are required because rust is dumb as of today (2024/01/16)
}
