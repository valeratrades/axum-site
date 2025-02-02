use futures::future::join_all;
use v_exchanges::{
	binance::{self, data::Lsrs},
	prelude::*,
};
use v_utils::prelude::*;

use crate::Mock;

const SLICE_SIZE: usize = 10;

//Q: potentially fix to "1D", req and store full month of data for both Global and Top Positions, to display when searching for specific one.
pub async fn get(tf: Timeframe, range: RequestRange) -> Result<String> {
	let mut bn = binance::Binance::default();
	bn.set_max_tries(3);

	let m = "Binance/Futures".into();
	let pairs = bn.exchange_info(m).await.unwrap().usdt_pairs().collect::<Vec<_>>();
	let pairs_len = pairs.len();

	let lsr_no_data_pairs_file = share_dir!().join("lsr_no_data_pairs");
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

	let lsrs: Vec<Lsrs> = results.into_iter().flatten().collect();
	let sorted_lsrs = SortedLsrs::build(lsrs);

	let mut s = String::new();
	let display_rows_ceiling = std::cmp::min(SLICE_SIZE, sorted_lsrs.len() / 2 /*floor*/);
	let width = Lsrs::CHANGE_STR_LEN;
	for i in 0..display_rows_ceiling {
		if i == 0 {
			let title = |t: &'static str| -> std::fmt::Arguments {
				//SAFETY: `t` is literally static
				unsafe { std::mem::transmute::<std::fmt::Arguments, std::fmt::Arguments>(format_args!("{:<width$}", format!("Most {t} (% longs)"), width = width)) }
			};

			s.write_fmt(format_args!("{}{}", title("Shorted"), title("Longed"))).unwrap(); // match formatting of `fmt_lsr` (when counting, don't forget all symbols outside of main paddings)
		} else {
			s.push('\n');
		}
		s.push_str(&sorted_lsrs.display_most_shorted_longed_row(i)?);
	}
	s.push_str(&format!("\n{:-^width$}", "", width = width));
	s.push_str(&format!(
		"\nAverage: {:.2}",
		sorted_lsrs.iter().map(|lsr| lsr.last().unwrap().long()).sum::<f64>() / sorted_lsrs.len() as f64
	));
	s.push_str(&format!("\nCollected for {}/{pairs_len} pairs on {}", sorted_lsrs.len(), m));
	Ok(s)
}

/// Inner values are guaranteed to be sorted
#[derive(Clone, Debug, derive_more::Deref, derive_more::DerefMut, Deserialize, Serialize)]
pub struct SortedLsrs {
	v: Vec<Lsrs>,
}
impl SortedLsrs {
	pub fn build(mut v: Vec<Lsrs>) -> Self {
		v.sort_by(|a, b| a.last().unwrap().long().partial_cmp(&b.last().unwrap().long()).unwrap());
		Self { v }
	}

	pub fn display_most_shorted_longed_row(&self, i: usize) -> Result<String> {
		if self.len() < 2 * i {
			bail!("Not enough data");
		}
		let shorted_str = self.get(i).expect("checked earlier").display_change()?;
		let longed_str = self.get(self.len() - i - 1).expect("checked earlier").display_change()?;
		Ok(format!("{shorted_str}{longed_str}"))
	}
}
impl Mock for SortedLsrs {
	const NAME: &'static str = "lsrs";
}
