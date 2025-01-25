use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::{America::New_York, Tz};
use v_utils::prelude::*;

static CFTC_CODE_BTC: u32 = 133741;

#[allow(unused)]
#[derive(Clone, Debug, Default, derive_new::new, Copy)]
struct PositionsInfo {
	current: f64,
	change_since_last_week: f64,
	percent_of_open: f64,
	number_of_traders: Option<u32>,
}

#[allow(unused)]
#[derive(Clone, Debug, Default, derive_new::new, Copy)]
struct Positions {
	long: PositionsInfo,
	short: PositionsInfo,
	spreading: PositionsInfo,
}

#[allow(unused)]
#[derive(Clone, Debug, Default, derive_new::new)]
struct CftcReport {
	// pub asset: String,
	pub date: DateTime<Utc>,
	pub dealer_intermidiary: Positions,
	pub asset_manager_or_institutional: Positions,
	pub leveraged_funds: Positions,
	pub other_reportables: Positions,
	_non_reportables: Option<Value>,
}
impl CftcReport {
	pub fn parse_by_index(page: &[String], index: u32) -> Result<Self> {
		let index_line_pos = page
			.iter()
			.position(|line| line.contains(&format!("#{}", index)))
			.ok_or_else(|| eyre!("Could not find the block with BTC index /*{CFTC_CODE_BTC}*/ in parsed CFTC report"))?;
		let block: &[String; 20] = page
			.get((index_line_pos - 8)..=(index_line_pos + 11))
			.and_then(|slice| slice.try_into().ok())
			.ok_or_else(|| eyre!("Block size mismatch - expected 20 lines"))?;

		block.try_into()
	}

	pub fn to_markdown_table(&self) -> String {
		let format_num = |n: f64| format!("{:.0}", n);
		let format_pct = |n: f64| format!("{:.1}", n);
		let format_trader = |n: Option<u32>| n.map_or(".".to_string(), |v| v.to_string());

		let date_str = self.date.format("%B %d, %Y").to_string();

		format!(
			"# Traders in Financial Futures - Futures Only Positions as of {}\n\n\
           |Position Type|Dealer Intermediary|||Asset Manager/Institutional|||Leveraged Funds|||Other Reportables|||\n\
           |------------|-------------------|---|---|----------------------|---|---|--------------|---|---|-----------------|---|---|\n\
           ||Long|Short|Spread|Long|Short|Spread|Long|Short|Spread|Long|Short|Spread|\n\
           |**Current**|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|\n\
           |**Changes**|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|\n\
           |**% of Open**|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|\n\
           |**Traders**|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|",
			date_str,
			// Current positions
			format_num(self.dealer_intermidiary.long.current),
			format_num(self.dealer_intermidiary.short.current),
			format_num(self.dealer_intermidiary.spreading.current),
			format_num(self.asset_manager_or_institutional.long.current),
			format_num(self.asset_manager_or_institutional.short.current),
			format_num(self.asset_manager_or_institutional.spreading.current),
			format_num(self.leveraged_funds.long.current),
			format_num(self.leveraged_funds.short.current),
			format_num(self.leveraged_funds.spreading.current),
			format_num(self.other_reportables.long.current),
			format_num(self.other_reportables.short.current),
			format_num(self.other_reportables.spreading.current),
			// Changes
			format_num(self.dealer_intermidiary.long.change_since_last_week),
			format_num(self.dealer_intermidiary.short.change_since_last_week),
			format_num(self.dealer_intermidiary.spreading.change_since_last_week),
			format_num(self.asset_manager_or_institutional.long.change_since_last_week),
			format_num(self.asset_manager_or_institutional.short.change_since_last_week),
			format_num(self.asset_manager_or_institutional.spreading.change_since_last_week),
			format_num(self.leveraged_funds.long.change_since_last_week),
			format_num(self.leveraged_funds.short.change_since_last_week),
			format_num(self.leveraged_funds.spreading.change_since_last_week),
			format_num(self.other_reportables.long.change_since_last_week),
			format_num(self.other_reportables.short.change_since_last_week),
			format_num(self.other_reportables.spreading.change_since_last_week),
			// Percentages
			format_pct(self.dealer_intermidiary.long.percent_of_open),
			format_pct(self.dealer_intermidiary.short.percent_of_open),
			format_pct(self.dealer_intermidiary.spreading.percent_of_open),
			format_pct(self.asset_manager_or_institutional.long.percent_of_open),
			format_pct(self.asset_manager_or_institutional.short.percent_of_open),
			format_pct(self.asset_manager_or_institutional.spreading.percent_of_open),
			format_pct(self.leveraged_funds.long.percent_of_open),
			format_pct(self.leveraged_funds.short.percent_of_open),
			format_pct(self.leveraged_funds.spreading.percent_of_open),
			format_pct(self.other_reportables.long.percent_of_open),
			format_pct(self.other_reportables.short.percent_of_open),
			format_pct(self.other_reportables.spreading.percent_of_open),
			// Traders
			format_trader(self.dealer_intermidiary.long.number_of_traders),
			format_trader(self.dealer_intermidiary.short.number_of_traders),
			format_trader(self.dealer_intermidiary.spreading.number_of_traders),
			format_trader(self.asset_manager_or_institutional.long.number_of_traders),
			format_trader(self.asset_manager_or_institutional.short.number_of_traders),
			format_trader(self.asset_manager_or_institutional.spreading.number_of_traders),
			format_trader(self.leveraged_funds.long.number_of_traders),
			format_trader(self.leveraged_funds.short.number_of_traders),
			format_trader(self.leveraged_funds.spreading.number_of_traders),
			format_trader(self.other_reportables.long.number_of_traders),
			format_trader(self.other_reportables.short.number_of_traders),
			format_trader(self.other_reportables.spreading.number_of_traders),
		)
	}
}
impl TryFrom<&[String; 20]> for CftcReport {
	type Error = Report;

	fn try_from(block: &[String; 20]) -> Result<Self> {
		let date = {
			let date_line = &block[1];
			let date_str = match date_line.find("as of") {
				Some(pos) => &date_line[pos + 6..].trim(),
				None => bail!("Date not found"),
			};
			let naive_date = chrono::NaiveDate::parse_from_str(date_str, "%B %d, %Y").map_err(|e| eyre!("Failed to parse date: {}", e))?;

			let eastern_time = naive_date.and_hms_opt(15, 30, 0).ok_or_else(|| eyre!("Failed to create time"))?;

			let eastern_datetime: DateTime<Tz> = New_York
				.from_local_datetime(&eastern_time)
				.earliest()
				.ok_or_else(|| eyre!("Failed to create Eastern timezone datetime"))?;
			eastern_datetime.with_timezone(&Utc)
		};

		fn parse_nums<T: std::str::FromStr + std::fmt::Display + std::fmt::Debug>(line: &str) -> Result<Vec<T>>
		where
			<T as std::str::FromStr>::Err: std::fmt::Display, {
			line.split_whitespace()
				.filter(|s| !s.is_empty())
				.map(|s| s.replace(",", "").parse::<T>())
				.collect::<std::result::Result<Vec<_>, _>>()
				.map_err(|e| eyre!("Failed to parse numbers: {}\nIn line: {line}", e))
		}

		let positions = parse_nums::<f64>(&block[10])?;
		let changes = parse_nums::<f64>(&block[13])?;
		let percents = parse_nums::<f64>(&block[16])?;
		let traders: &Vec<Option<u32>> = &block[19]
			.split_whitespace()
			.filter(|s| !s.is_empty())
			.map(|s| if s == "." { None } else { s.parse().ok() })
			.collect();

		fn create_positions(start_idx: usize, positions: &[f64], changes: &[f64], percents: &[f64], traders: &[Option<u32>]) -> Result<Positions> {
			Ok(Positions {
				long: PositionsInfo::new(positions[start_idx], changes[start_idx], percents[start_idx], traders[start_idx]),
				short: PositionsInfo::new(positions[start_idx + 1], changes[start_idx + 1], percents[start_idx + 1], traders[start_idx + 1]),
				spreading: PositionsInfo::new(positions[start_idx + 2], changes[start_idx + 2], percents[start_idx + 2], traders[start_idx + 2]),
			})
		}

		Ok(CftcReport {
			date,
			dealer_intermidiary: create_positions(0, &positions, &changes, &percents, traders)?,
			asset_manager_or_institutional: create_positions(3, &positions, &changes, &percents, traders)?,
			leveraged_funds: create_positions(6, &positions, &changes, &percents, traders)?,
			other_reportables: create_positions(9, &positions, &changes, &percents, traders)?,
			_non_reportables: None,
		})
	}
}

#[derive(Clone, Debug, Default, derive_new::new, Copy)]
struct DirectionalPositionsChange {
	name: &'static str,
	long: NowThen,
	short: NowThen,
}
impl PrettyPrint for DirectionalPositionsChange {
	fn pretty(&self, f: &mut std::fmt::Formatter<'_>, root_indent: u8) -> std::fmt::Result {
		writeln!(f, "{}:", self.name)?;
		let indent = " ".repeat(root_indent as usize);
		write!(f, "{indent}Long: {}, Short: {}", self.long, self.short)
	}
}

pub async fn fetch_cftc_positions() -> Result<String> {
	let url = "https://www.cftc.gov/dea/futures/financial_lf.htm";
	let response = reqwest::get(url).await?.text().await?;
	let lines: Vec<String> = response.lines().map(String::from).collect();

	let report = CftcReport::parse_by_index(&lines, CFTC_CODE_BTC)?;
	let institutional_change = {
		let institutional = &report.asset_manager_or_institutional;
		DirectionalPositionsChange {
			name: "Institutional",
			long: NowThen::from_now_diff(institutional.long.current, institutional.long.change_since_last_week),
			short: NowThen::from_now_diff(institutional.short.current, institutional.short.change_since_last_week),
		}
	};
	let funds_change = {
		let funds = &report.leveraged_funds;
		DirectionalPositionsChange {
			name: "Hedgefunds",
			long: NowThen::from_now_diff(funds.long.current, funds.long.change_since_last_week),
			short: NowThen::from_now_diff(funds.short.current, funds.short.change_since_last_week),
		}
	};

	Ok(format!(
		"CME positions as of {}:\n- {}\n- {}",
		report.date,
		institutional_change.to_string_pretty(2),
		funds_change.to_string_pretty(2)
	))
}

#[allow(unused)]
#[tokio::main]
#[doc(hidden)]
/// for testing purposes
async fn main() {
	clientside!();

	match fetch_cftc_positions().await {
		Ok(result) => println!("{}", result),
		Err(e) => eprintln!("Error: {}", e),
	}
}
