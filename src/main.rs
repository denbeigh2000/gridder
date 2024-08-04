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

fn prepare_csv_path(d: &NaiveDate, template: &str, key: &str) -> PathBuf {
    let csv_name = d.format(&template.replace("_ITEM_", key)).to_string();
    let csv_path = PathBuf::from(&csv_name);
    let dirname = csv_path.parent().unwrap();
    if !dirname.exists() {
        // TODO: return Result<>
        std::fs::create_dir_all(dirname).expect("failed to make dir");
    }

    dirname
        .canonicalize()
        .expect("failed to canonicalize path")
        .join(csv_path.file_name().unwrap())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let today = match args.date {
        Some(input_str) => input_str.parse().expect("failed to parse time"),
        None => chrono::Utc::now().with_timezone(&US_WEST_TZ).date_naive(),
    };

    let prefix = String::from_utf8_lossy(&STR_URL_PREFIX);
    let suffix = String::from_utf8_lossy(&STR_URL_SUFFIX);
    let date_str = today.format("%Y/%m/%d");
    let url_str = format!("{prefix}/{date_str}/{suffix}");

    // TODO: subtle user agent?
    let resp = reqwest::get(url_str)
        .await
        .expect("failed to get url")
        .error_for_status()
        .expect("bad status from resp");

    let body = resp.text().await.expect("failed to read body");
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

    // NOTE: can't use an unwrap_or here due to lifetime requirements
    let template = args.filename_format.as_deref().unwrap_or(DEFAULT_FORMAT);
    let lengths_path = prepare_csv_path(&today, template, "lengths");
    let mut writer = csv::Writer::from_path(&lengths_path).expect("failed to open file");

    for ((letter, len), quantity) in table_info.iter() {
        // NOTE: csv writer expects these to be representable as &[u8], even if
        // writing individual records, so we still need to convert these to
        // strings.
        let record = [letter.to_string(), len.to_string(), quantity.to_string()];
        writer
            .write_record(&record)
            .expect("failed to write length record");
    }

    let pairs_path = prepare_csv_path(&today, template, "pairs");
    let mut writer = csv::Writer::from_path(&pairs_path).expect("failed to open pairs file");
    for ((a, b), v) in pairs.iter() {
        let record = [format!("{a}{b}"), v.to_string()];
        writer
            .write_record(record)
            .expect("failed to write pair record");
    }

    eprintln!("operation success!");
    eprintln!("pairs written to:   {}", pairs_path.to_string_lossy());
    eprintln!("lengths written to: {}", lengths_path.to_string_lossy());

    eprintln!();
    eprintln!("instructions:\n---");

    eprintln!("import length CSV to B3");
    eprintln!("import pair   CSV to F3");
    eprintln!("remember to replace cell data!");
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
