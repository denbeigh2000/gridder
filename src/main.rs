use chrono::NaiveDate;
use chrono_tz::Tz;
use clap::Parser;

use std::path::PathBuf;

use gridder::fetch::{fetch_for_date, FetchDataError};
use gridder::parse::parse_content;

// New releases happen at midnight US-West time
const US_WEST_TZ: Tz = chrono_tz::America::Los_Angeles;

const DEFAULT_FORMAT: &str = "./%Y-%m-%d-_ITEM_.csv";

#[derive(clap::Parser, Debug)]
struct Args {
    /// The date to retrieve data for.
    /// Format: YYYY-MM-DD
    date: Option<String>,

    #[arg(short, long)]
    /// The format of the filename to write files to.
    /// _ITEM_ will be replaced with "pairs" or "lengths".
    filename_format: Option<String>,
}

#[derive(thiserror::Error, Debug)]
enum PreparingCSVPathError {
    #[error("filename template must not end in a slash ({0})")]
    GivenDirectory(PathBuf),
    #[error("{0} already exists as a directory")]
    ExistsAsDirectory(PathBuf),
    #[error("failed to mkdir {0} ({1})")]
    Mkdir(PathBuf, std::io::Error),
    #[error("failed to canonicalise {0} ({1})")]
    CanonicalisingPath(PathBuf, std::io::Error),
}

fn prepare_csv_path(
    d: &NaiveDate,
    template: &str,
    key: &str,
) -> Result<PathBuf, PreparingCSVPathError> {
    let t = match template {
        "" => DEFAULT_FORMAT,
        t => t,
    };
    let csv_name = d.format(&t.replace("_ITEM_", key)).to_string();
    let csv_path = PathBuf::from(&csv_name);

    let dirname = match csv_path.parent() {
        Some(d) => d,
        // If somebody really wants to write to /, that's okay I guess.
        None => return Ok(csv_path),
    };

    if !dirname.exists() {
        std::fs::create_dir_all(dirname)
            .map_err(|e| PreparingCSVPathError::Mkdir(dirname.to_path_buf(), e))?
    } else if csv_path.is_dir() {
        return Err(PreparingCSVPathError::ExistsAsDirectory(csv_path));
    }

    match csv_path.file_name() {
        Some(tpl_filename) => {
            let path = dirname
                .canonicalize()
                .map_err(|e| PreparingCSVPathError::CanonicalisingPath(dirname.to_path_buf(), e))?
                .join(tpl_filename);

            Ok(path)
        }
        None => Err(PreparingCSVPathError::GivenDirectory(csv_path)),
    }
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("failed to parse {0} into a date ({1})")]
    ParsingDate(String, chrono::ParseError),
    #[error("failed to fetch site data: {0}")]
    FetchingSiteData(#[from] FetchDataError),
    #[error("error preparing CSV path for {0} ({1})")]
    PreparingCSVPath(&'static str, PreparingCSVPathError),
    #[error("error opening ouptut file for {0} ({1}")]
    OpeningCSVFile(&'static str, csv::Error),
    #[error("error writing output line for {0} ({1})")]
    WritingCSVRecord(&'static str, csv::Error),
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

    let template = args.filename_format.as_deref().unwrap_or(DEFAULT_FORMAT);
    let lengths_path = prepare_csv_path(&date, template, "lengths")
        .map_err(|e| Error::PreparingCSVPath("lengths", e))?;
    let mut writer = csv::Writer::from_path(&lengths_path)
        .map_err(|err| Error::OpeningCSVFile("lengths", err))?;

    for ((letter, len), quantity) in table_info.iter() {
        // NOTE: csv writer expects these to be representable as &[u8], even if
        // writing individual records, so we still need to convert these to
        // strings.
        let record = [letter.to_string(), len.to_string(), quantity.to_string()];
        writer
            .write_record(&record)
            .map_err(|e| Error::WritingCSVRecord("lengths", e))?;
    }

    let pairs_path = prepare_csv_path(&date, template, "pairs")
        .map_err(|e| Error::PreparingCSVPath("pairs", e))?;
    let mut writer = csv::Writer::from_path(&pairs_path)
        .map_err(|error| Error::OpeningCSVFile("pairs", error))?;
    for ((a, b), v) in pairs.iter() {
        let record = [format!("{a}{b}"), v.to_string()];
        writer
            .write_record(record)
            .map_err(|e| Error::WritingCSVRecord("pairs", e))?;
    }

    eprintln!("operation success!");
    eprintln!("pairs written to:   {}", pairs_path.to_string_lossy());
    eprintln!("lengths written to: {}", lengths_path.to_string_lossy());

    eprintln!();
    eprintln!("instructions:\n---");

    eprintln!("import length CSV to B3");
    eprintln!("import pair   CSV to F3");
    eprintln!("remember to replace cell data!");

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = real_main().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
