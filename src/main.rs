use chrono_tz::Tz;
use clap::Parser;
use gridder::sheets::{NewSheetError, SheetCreationError, SheetManager};

use std::path::PathBuf;

use gridder::fetch::{fetch_for_date, FetchDataError};
use gridder::parse::parse_content;

// New releases happen at midnight US-West time
const US_WEST_TZ: Tz = chrono_tz::America::Los_Angeles;

#[derive(clap::Parser, Debug)]
struct Args {
    /// The date to retrieve data for.
    /// Format: YYYY-MM-DD
    date: Option<String>,

    #[arg(short = 'i', long, env = "GRIDDER_SPREADSHEET_ID")]
    spreadsheet_id: String,

    #[arg(short = 'p', long, env = "GRIDDER_SERVICE_ACCOUNT_FILE")]
    service_account_file: PathBuf,
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("failed to parse {0} into a date ({1})")]
    ParsingDate(String, chrono::ParseError),
    #[error("failed to fetch site data: {0}")]
    FetchingSiteData(#[from] FetchDataError),
    #[error("failed to create Sheets API client: {0}")]
    CreatingSheetManager(#[from] NewSheetError),
    #[error("failed to create new daily sheet: {0}")]
    UpdatingSpreadsheet(#[from] SheetCreationError),
}

async fn real_main() -> Result<(), Error> {
    let args = Args::parse();
    let date = args
        .date
        // If a datestring was given, try to parse it into a NaiveDate
        .map(|i| i.parse().map_err(|e| Error::ParsingDate(i, e)))
        // Put the Result<..> on the outside, and exit if it failed
        .transpose()?
        // If no date was given, fall back to using today (in US-Western)
        .unwrap_or_else(|| chrono::Utc::now().with_timezone(&US_WEST_TZ).date_naive());

    let body = fetch_for_date(date).await?;
    let (pairs, table_info) = parse_content(&body).expect("failed to extract info from document");

    let sheets_client = SheetManager::new(&args.spreadsheet_id, args.service_account_file).await?;
    sheets_client
        .create_for_date(&date, &pairs, &table_info)
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = real_main().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
