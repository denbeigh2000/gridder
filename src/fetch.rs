use std::fmt::write;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use chrono::NaiveDate;
use user_agents::get_user_agents;

use rand::random_range;

const URL_PREFIX: &str = "aHR0cHM6Ly93d3cubnl0aW1lcy5jb20=";
const URL_SUFFIX: &str = "Y3Jvc3N3b3Jkcy9zcGVsbGluZy1iZWUtZm9ydW0uaHRtbA==";

lazy_static::lazy_static! {
    static ref STR_URL_PREFIX: Vec<u8> = BASE64_STANDARD.decode(URL_PREFIX).unwrap();
    static ref STR_URL_SUFFIX: Vec<u8> = BASE64_STANDARD.decode(URL_SUFFIX).unwrap();
}

#[derive(Debug, thiserror::Error)]
pub enum FetchDataError {
    #[error("error getting user agent: ({0})")]
    GettingUserAgent(#[from] user_agents::FetchError),
    #[error("failed to build http client: ({0})")]
    BuildingClient(reqwest::Error),
    #[error("failed to construct request: ({0})")]
    BuildingRequest(reqwest::Error),
    #[error("failed to get info page ({0})")]
    FetchingUrl(reqwest::Error),
    #[error("got bad http status from server ({0})")]
    BadResponse(reqwest::Error),
    #[error("failed to read response body ({0})")]
    ReadingBody(reqwest::Error),
}

async fn get_user_agent() -> Result<String, user_agents::FetchError> {
    let mut agents = get_user_agents().await?;
    let agent_set = agents.humans.get_mut("Mac OS;;Chrome").expect("No MacOS/chrome agent?");

    let agent_index = random_range(..agent_set.len());
    let agent = agent_set.remove(agent_index);

    Ok(agent.user_agent)
}

pub async fn fetch_for_date(date: NaiveDate) -> Result<String, FetchDataError> {
    let prefix = String::from_utf8_lossy(&STR_URL_PREFIX);
    let suffix = String::from_utf8_lossy(&STR_URL_SUFFIX);
    let date_str = date.format("%Y/%m/%d");
    let url_str = format!("{prefix}/{date_str}/{suffix}");

    let user_agent = get_user_agent().await?;

    let client = reqwest::Client::builder()
        .user_agent(user_agent)
        .build()
        .map_err(FetchDataError::BuildingClient)?;

    let req = client
        .get(url_str)
        .build()
        .map_err(FetchDataError::BuildingRequest)?;

    let resp = client
        .execute(req)
        .await
        .map_err(FetchDataError::FetchingUrl)?
        .error_for_status()
        .map_err(FetchDataError::BadResponse)?;

    resp.text().await.map_err(FetchDataError::ReadingBody)
}
