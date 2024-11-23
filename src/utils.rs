use std::{
	fs::File,
	io::Write,
	path::Path,
	sync::{atomic::Ordering, Arc},
	time::Duration,
};

use color_eyre::eyre::{bail, eyre, Report, Result, WrapErr};
use function_name::named;
use serde::{de::DeserializeOwned, Deserializer};


// Deser Reqwest {{{
fn deser_reqwest_core<T: DeserializeOwned>(text: String) -> Result<T> {
	match serde_json::from_str::<T>(&text) {
		Ok(deserialized) => Ok(deserialized),
		Err(e) => {
			let mut error_msg = e.to_string();
			if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&text) {
				//let _ = std::panic::catch_unwind(|| {
				//	dbg!(&json_value["symbols"][0]);
				//});

				let mut jd = serde_json::Deserializer::from_str(&text);
				let r: Result<T, _> = serde_path_to_error::deserialize(&mut jd);
				if let Err(e) = r {
					error_msg = e.path().to_string();
				}
			}
			Err(unexpected_response_str(&text)).wrap_err_with(|| error_msg)
		}
	}
}

/// Basically reqwest's `json()`, but prints the target's content on deserialization error.
pub async fn deser_reqwest<T: DeserializeOwned>(r: reqwest::Response) -> Result<T> {
	let text = r.text().await?;
	deser_reqwest_core(text)
}

pub fn unexpected_response_str(s: &str) -> Report {
	let s = match serde_json::from_str::<serde_json::Value>(s) {
		Ok(v) => serde_json::to_string_pretty(&v).unwrap(),
		Err(_) => s.to_owned(),
	};
	let report = report_msg(s);
	report.wrap_err("Unexpected API response")
}
//,}}}

/// Constructs `eyre::Report` with capped size
#[track_caller]
#[named]
pub fn report_msg(s: String) -> Report {
	let lines: Vec<&str> = s.lines().collect();
	let total_lines = lines.len();

	let truncated_message = if total_lines > 50 {
		let first_25 = &lines[..25];
		let last_25 = &lines[total_lines - 25..];
		let truncation_message = format!("------------------------- // truncated at {} by `{}`\n", std::panic::Location::caller(), function_name!());
		let concat_message = format!("{}\n{truncation_message}{}", first_25.join("\n"), last_25.join("\n"));

		concat_message
	} else {
		s
	};

	Report::msg(truncated_message)
}
