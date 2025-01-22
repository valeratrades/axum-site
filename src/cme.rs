use chrono::{DateTime, NaiveDateTime, TimeDelta, TimeZone, Utc};
use chrono_tz::{America::New_York, Tz};
use reqwest;
use v_utils::prelude_clientside::*;

static CFTC_CODE_BTC: usize = 133741;

#[derive(Debug)]
struct Settings {
	comparison_limit: Option<TimeDelta>,
}

#[derive(Debug)]
struct Position {
	long: i32,
	short: i32,
}

#[derive(Clone, Debug, Default, derive_new::new, Copy)]
struct PositionsInfo {
	current: u32,
	change_since_last_week: i32,
	percent_of_open: f32,
	number_of_traders: u32,
}

#[derive(Clone, Debug, Default, derive_new::new, Copy)]
struct Exposure {
	long: PositionsInfo,
	short: PositionsInfo,
	spreading: PositionsInfo,
}

//TODO!!!: parse entirety of the table into rust-native form \
#[derive(Clone, Debug, Default, derive_new::new)]
struct CftcReport {
	// pub asset: String,
	pub date: DateTime<Utc>,
	pub dealer_intermidiary: Exposure,
	pub asset_manager_or_institutional: Exposure,
	pub leveraged_funds: Exposure,
	pub other_reportables: Exposure,
	_non_reportables: String,
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

		let positions_line = &block[10];
		let change_line = &block[13];

		Ok(CftcReport {
			date,
			positions_line: positions_line.to_string(),
			change_line: change_line.to_string(),
		})
	}
}

#[derive(Debug)]
struct PositionChange {
	positions: Position,
	change: Position,
}

fn collect_positions(positions_line: &str, change_line: &str, index: usize) -> PositionChange {
	fn parse_numbers(line: &str) -> Vec<i32> {
		line.split_whitespace().filter(|s| !s.is_empty()).map(|s| s.replace(",", "").parse::<i32>().unwrap()).collect()
	}

	let p_numbers = parse_numbers(positions_line);
	let c_numbers = parse_numbers(change_line);

	PositionChange {
		positions: Position {
			long: p_numbers[index],
			short: p_numbers[index + 1],
		},
		change: Position {
			long: c_numbers[index],
			short: c_numbers[index + 1],
		},
	}
}

fn format_position(numbers: &PositionChange, settings: &Settings) -> String {
	let format_number = |pos: i32, change: i32| -> String {
		if settings.comparison_limit.is_some() {
			format!("{}{:+}", pos, change)
		} else {
			pos.to_string()
		}
	};

	format!(
		"({}, {})",
		format_number(numbers.positions.long, numbers.change.long),
		format_number(numbers.positions.short, numbers.change.short)
	)
}

async fn fetch_cftc_positions(settings: Settings) -> Result<String> {
	let url = "https://www.cftc.gov/dea/futures/financial_lf.htm";
	let response = reqwest::get(url).await?.text().await?;
	let lines: Vec<String> = response.lines().map(String::from).collect();

	///DEPENDS: relies on the structure of the CFTC report page
	fn parse_cme_block_by_index<'a>(index: usize, source: &'a [String]) -> Result<&'a [String; 20]> {
		let index_line = source
			.iter()
			.position(|line| line.contains(&format!("#{}", index)))
			.ok_or_else(|| eyre!("Could not find the block with BTC index /*{CFTC_CODE_BTC}*/ in parsed CFTC report"))?;

		source
			.get((index_line - 8)..=(index_line + 11))
			.and_then(|slice| slice.try_into().ok())
			.ok_or_else(|| eyre!("Block size mismatch - expected 20 lines"))
	}
	let block = parse_cme_block_by_index(CFTC_CODE_BTC, &lines)?;

	let mut start_index = 0;
	for (i, line) in lines.iter().enumerate() {
		if line.contains("#133741") {
			start_index = i;
			break;
		}
	}

	let cftc_report = CftcReport::try_from(block)?;
	let positions_line = &cftc_report.positions_line;
	let change_line = &cftc_report.change_line;
	dbg!(&block);
	dbg!(&cftc_report);
	//let positions_line = &block[10];
	//let change_line = &block[13];

	//DO: deserialize the data to CftcReport struct
	//DO: display the current formatted diffs, but make it a button that shows the full data (solves the annotations issues, as if I forget what each means, showing the full sections > custom naming)

	let institutional = collect_positions(positions_line, change_line, 3);
	let leveraged_funds = collect_positions(positions_line, change_line, 6);

	fn find_date(block: &[String; 20]) -> Result<String> {
		let date_line = &block[1];
		if let Some(pos) = date_line.find("as of") {
			Ok(date_line[pos + 6..pos + 14].to_string())
		} else {
			bail!("Date not found")
		}
	}
	let from_date = find_date(block);

	Ok(format!(
		"__CME positions; {}:\n{} {}",
		from_date?,
		format_position(&institutional, &settings),
		format_position(&leveraged_funds, &settings)
	))
}

#[tokio::main]
async fn main() {
	clientside!();
	let settings = Settings {
		comparison_limit: Some(TimeDelta::days(1)),
	};

	match fetch_cftc_positions(settings).await {
		Ok(result) => println!("{}", result),
		Err(e) => eprintln!("Error: {}", e),
	}
}
