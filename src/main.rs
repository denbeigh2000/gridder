use base64::{prelude::BASE64_STANDARD, Engine};
use chrono::NaiveDate;
use chrono_tz::Tz;
use clap::Parser;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};

use std::collections::HashMap;
use std::path::PathBuf;

// New releases happen at midnight US-West time
const US_WEST_TZ: Tz = chrono_tz::America::Los_Angeles;

const URL_PREFIX: &str = "aHR0cHM6Ly93d3cubnl0aW1lcy5jb20=";
const URL_SUFFIX: &str = "Y3Jvc3N3b3Jkcy9zcGVsbGluZy1iZWUtZm9ydW0uaHRtbA==";

const DEFAULT_FORMAT: &str = "./%Y-%m-%d-_ITEM_.csv";

lazy_static::lazy_static! {
    static ref TABLE_SELECTOR: Selector = Selector::parse("table.table").unwrap();
    static ref TR_SELECTOR: Selector = Selector::parse("tr.row").unwrap();
    static ref TD_SELECTOR: Selector = Selector::parse("td.cell").unwrap();
    static ref CONTENT_SELECTOR: Selector = Selector::parse("p.content").unwrap();

    static ref STR_URL_PREFIX: Vec<u8> = BASE64_STANDARD.decode(URL_PREFIX).unwrap();
    static ref STR_URL_SUFFIX: Vec<u8> = BASE64_STANDARD.decode(URL_SUFFIX).unwrap();

    static ref TWO_LETTER_REGEX: Regex = Regex::new(r#"\b([a-zA-Z]{2})-(\d+)\b"#).unwrap();
}

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
    #[error("failed to get info page ({0})")]
    FetchingUrl(reqwest::Error),
    #[error("got bad http status from server ({0})")]
    BadResponse(reqwest::Error),
    #[error("failed to read response body ({0})")]
    ReadingBody(reqwest::Error),
    #[error("error preparing CSV path for {0} ({1})")]
    PreparingCSVPath(&'static str, PreparingCSVPathError),
    #[error("error opening ouptut file for {0} ({1}")]
    OpeningCSVFile(&'static str, csv::Error),
    #[error("error writing output line for {0} ({1})")]
    WritingCSVRecord(&'static str, csv::Error),
}

async fn real_main() -> Result<(), Error> {
    let args = Args::parse();
    let today = match args.date {
        Some(input_str) => input_str
            .parse()
            .map_err(|e| Error::ParsingDate(input_str, e))?,
        None => chrono::Utc::now().with_timezone(&US_WEST_TZ).date_naive(),
    };

    let prefix = String::from_utf8_lossy(&STR_URL_PREFIX);
    let suffix = String::from_utf8_lossy(&STR_URL_SUFFIX);
    let date_str = today.format("%Y/%m/%d");
    let url_str = format!("{prefix}/{date_str}/{suffix}");

    // TODO: subtle user agent?
    let resp = reqwest::get(url_str)
        .await
        .map_err(Error::FetchingUrl)?
        .error_for_status()
        .map_err(Error::BadResponse)?;

    let body = resp.text().await.map_err(Error::ReadingBody)?;
    let page = Html::parse_document(&body);

    let table = match page.select(&TABLE_SELECTOR).next() {
        Some(i) => i,
        None => panic!("missing table on page"),
    };

    let main_node = table.parent().unwrap();
    let main_el = ElementRef::wrap(main_node).unwrap();

    let two_letters_el = main_el.select(&CONTENT_SELECTOR).nth(4).unwrap();

    let pairs = extract_pair_info(two_letters_el);
    let table_info = extract_table_info(table);

    let template = args.filename_format.as_deref().unwrap_or(DEFAULT_FORMAT);
    let lengths_path = prepare_csv_path(&today, template, "lengths")
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

    let pairs_path = prepare_csv_path(&today, template, "pairs")
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

fn extract_pair_info(node: ElementRef) -> HashMap<(char, char), usize> {
    let text_vec = node.text().collect::<Vec<_>>();
    let text = text_vec.concat();

    let mut pair_counts = HashMap::default();
    for (_, [prefix, count]) in TWO_LETTER_REGEX.captures_iter(&text).map(|c| c.extract()) {
        assert!(prefix.len() == 2);
        let i: usize = count.parse().expect("received negative count");
        let mut chars = prefix.chars();
        let char1 = chars.next().unwrap();
        let char2 = chars.next().unwrap();
        pair_counts.insert((char1, char2), i);
    }

    pair_counts
}

fn extract_table_info(node: ElementRef) -> HashMap<(char, usize), usize> {
    let mut rows = node.select(&TR_SELECTOR);
    // Expecting 8 rows: 1 header, 6 letters, 1 sum
    let header = rows.next().unwrap();
    let (_, values) = extract_table_row_info(header);

    let mut items = HashMap::default();
    for row in rows {
        let (l, quants) = extract_table_row_info(row);
        let letter = l.unwrap();
        if letter == 'Σ' {
            continue;
        }

        for (i, quantity) in quants.iter().enumerate() {
            items.insert((letter, values[i]), *quantity);
        }
    }

    items
}

fn extract_table_row_info(tr: ElementRef) -> (Option<char>, Vec<usize>) {
    let mut els = tr.select(&TD_SELECTOR);
    let header = els.next().unwrap().text().collect::<Vec<_>>().concat();
    let header_char = header.trim().chars().next();

    let mut items = Vec::new();
    for el in els {
        let text = el.text().collect::<Vec<_>>().concat();
        let num = match text.trim() {
            // This doesn't matter, and will get dropped just below anyway
            "Σ" => 0,
            "-" => 0,
            v => v.parse().unwrap(),
        };
        items.push(num);
    }

    // drop the "sum" item
    items.truncate(items.len() - 1);
    (header_char, items)
}
