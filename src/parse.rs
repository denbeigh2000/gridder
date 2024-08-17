use std::collections::HashMap;

use regex::Regex;
use scraper::{ElementRef, Html, Selector};

use crate::{LengthInfo, PairInfo};

lazy_static::lazy_static! {
    static ref TABLE_SELECTOR: Selector = Selector::parse("table.table").unwrap();
    static ref TR_SELECTOR: Selector = Selector::parse("tr.row").unwrap();
    static ref TD_SELECTOR: Selector = Selector::parse("td.cell").unwrap();
    static ref CONTENT_SELECTOR: Selector = Selector::parse("p.content").unwrap();

    static ref TWO_LETTER_REGEX: Regex = Regex::new(r#"\b([a-zA-Z]{2})-(\d+)\b"#).unwrap();
}

#[derive(Debug, thiserror::Error)]
pub enum SiteParseError {}

pub fn parse_content(body: &str) -> Result<(PairInfo, LengthInfo), SiteParseError> {
    let page = Html::parse_document(body);

    let table = match page.select(&TABLE_SELECTOR).next() {
        Some(i) => i,
        None => panic!("missing table on page"),
    };

    let main_node = table.parent().unwrap();
    let main_el = ElementRef::wrap(main_node).unwrap();

    let two_letters_el = main_el.select(&CONTENT_SELECTOR).nth(4).unwrap();

    let pairs = extract_pair_info(two_letters_el);
    let table_info = extract_table_info(table);

    Ok((pairs, table_info))
}

fn extract_pair_info(node: ElementRef) -> PairInfo {
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

fn extract_table_info(node: ElementRef) -> LengthInfo {
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
