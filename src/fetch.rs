use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use chrono::NaiveDate;
use cuimp::CuimpOptions;

use crate::user_agents::{get_user_agent, UserAgentConstructionError};

const URL_PREFIX: &str = "aHR0cHM6Ly93d3cubnl0aW1lcy5jb20=";
const URL_SUFFIX: &str = "Y3Jvc3N3b3Jkcy9zcGVsbGluZy1iZWUtZm9ydW0uaHRtbA==";

lazy_static::lazy_static! {
    static ref STR_URL_PREFIX: Vec<u8> = BASE64_STANDARD.decode(URL_PREFIX).unwrap();
    static ref STR_URL_SUFFIX: Vec<u8> = BASE64_STANDARD.decode(URL_SUFFIX).unwrap();
}

#[derive(Debug, thiserror::Error)]
pub enum FetchDataError {
    #[error("error building client: {0}")]
    BuildingClient(cuimp::CuimpError),
    #[error("error getting user agent: ({0})")]
    GettingUserAgent(#[from] UserAgentConstructionError),
    #[error("error fetching NYT game page: {0}")]
    GettingData(#[from] cuimp::CuimpError),
}

pub async fn fetch_for_date(date: NaiveDate) -> Result<String, FetchDataError> {
    let prefix = String::from_utf8_lossy(&STR_URL_PREFIX);
    let suffix = String::from_utf8_lossy(&STR_URL_SUFFIX);
    let date_str = date.format("%Y/%m/%d");
    let url_str = format!("{prefix}/{date_str}/{suffix}");
    let user_agent = get_user_agent().await?;
    let curl_args = [
        "--compressed".to_string(),
        "--header".to_string(),
        format!("User-Agent: {user_agent}"),
        "--header".to_string(),
        "Accept: text/html".to_string(),
        "--header".to_string(),
        "Accept-Language: en-US".to_string(),
    ];
    let options = CuimpOptions {
        extra_curl_args: Some(curl_args.into()),
        ..Default::default()
    };
    let mut client = cuimp::CuimpHttp::new(options).map_err(FetchDataError::BuildingClient)?;

    let resp = client.get(&url_str).await?;
    Ok(resp.data)
}
