use base64::{prelude::BASE64_STANDARD, Engine};
use chrono::NaiveDate;

const URL_PREFIX: &str = "aHR0cHM6Ly93d3cubnl0aW1lcy5jb20=";
const URL_SUFFIX: &str = "Y3Jvc3N3b3Jkcy9zcGVsbGluZy1iZWUtZm9ydW0uaHRtbA==";

lazy_static::lazy_static! {
    static ref STR_URL_PREFIX: Vec<u8> = BASE64_STANDARD.decode(URL_PREFIX).unwrap();
    static ref STR_URL_SUFFIX: Vec<u8> = BASE64_STANDARD.decode(URL_SUFFIX).unwrap();
}

#[derive(Debug, thiserror::Error)]
pub enum FetchDataError {
    #[error("failed to get info page ({0})")]
    FetchingUrl(reqwest::Error),
    #[error("got bad http status from server ({0})")]
    BadResponse(reqwest::Error),
    #[error("failed to read response body ({0})")]
    ReadingBody(reqwest::Error),
}

pub async fn fetch_for_date(date: NaiveDate) -> Result<String, FetchDataError> {
    let prefix = String::from_utf8_lossy(&STR_URL_PREFIX);
    let suffix = String::from_utf8_lossy(&STR_URL_SUFFIX);
    let date_str = date.format("%Y/%m/%d");
    let url_str = format!("{prefix}/{date_str}/{suffix}");

    // TODO: subtle user agent?
    let resp = reqwest::get(url_str)
        .await
        .map_err(FetchDataError::FetchingUrl)?
        .error_for_status()
        .map_err(FetchDataError::BadResponse)?;

    resp.text().await.map_err(FetchDataError::ReadingBody)
}
