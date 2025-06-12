use base64::{prelude::BASE64_STANDARD, Engine};
use rand::seq::SliceRandom;

const CHROMIUM_RELEASE_JSON_URL: &str = "https://chromiumdash.appspot.com/fetch_releases";
const UA_PREFIX_1: &str = "TW96aWxsYS81LjAgQXBwbGVXZWJLaXQvNTM3LjM2IChLSFRNTCwgbGlrZSBHZWNrbzsgY29tcGF0aWJsZTsgRw==";
const UA_PREFIX_2: &str = "b29nbGVibw==";
const UA_PREFIX_3: &str = "dC8yLjE7ICtodHRwOi8vd3d3Lmdvb2dsZS5jb20vYm90Lmh0bWwpIENocm9tZS8=";
const UA_SUFFIX: &str = "IFNhZmFyaS81MzcuMzY=";

const TARGET_CHROMIUM_CHANNEL: &str = "Stable";
const TARGET_CHROMIUM_PLATFORM: &str = "Win32";

lazy_static::lazy_static! {
    static ref UA_PREFIX_1_STR: Vec<u8> = BASE64_STANDARD.decode(UA_PREFIX_1).unwrap();
    static ref UA_PREFIX_3_STR: Vec<u8> = BASE64_STANDARD.decode(UA_PREFIX_3).unwrap();
    static ref UA_SUFFIX_STR: Vec<u8> = BASE64_STANDARD.decode(UA_SUFFIX).unwrap();
}

/*
 * Sample JSON blob from the Chromium release manifest:
 * {
 *   "channel": "Canary_asan",
 *   "chromium_main_branch_position": 1167750,
 *   "hashes": {
 *     "angle": "20cc4a9bc250a009a88b3491083a86ca6d85b79b",
 *     "chromium": "63d73443f76660cdac9978b1736f93c18b243977",
 *     "dawn": "9fc22d10fd46ea696d5e6305bbfd8c0fc6b2bfbe",
 *     "devtools": "d12637511c19e5a3d060656eeb54e76e410715ca",
 *     "pdfium": "f181dc10ae4ec2c8ecbb025ceea74f936e03f1cd",
 *     "skia": "6d733caa0d3f7c5ba83af70dc3f64f2f011326aa",
 *     "v8": "bf2edb44253cbcc951c6a20a9dbbcb644fc1d91d",
 *     "webrtc": "104304724329915af50f9d14a2cc044ebee68d85"
 *   },
 *   "milestone": 117,
 *   "platform": "Webview",
 *   "previous_version": "117.0.5878.1",
 *   "time": 1688843910140,
 *   "version": "117.0.5879.1"
 * }
 */

#[derive(serde::Deserialize)]
pub struct ChromiumReleaseInfo {
    pub channel: String,
    pub version: String,
    pub platform: String,
}

impl ChromiumReleaseInfo {
    pub fn matches(&self, channel: &str, platform: &str) -> bool {
        self.channel == channel && self.platform == platform
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ChromiumReleaseInfoFetchError {
    #[error("error fetching release info: {0}")]
    MakingRequest(reqwest::Error),
    #[error("error reading response body: {0}")]
    ReadingBody(reqwest::Error),
    #[error("error decoding response into JSON: {0}")]
    DecodingChromiumReleaseInfo(#[from] serde_json::Error),
}

async fn fetch_chromium_releases() -> Result<Vec<ChromiumReleaseInfo>, ChromiumReleaseInfoFetchError>
{
    let resp = reqwest::get(CHROMIUM_RELEASE_JSON_URL)
        .await
        .map_err(ChromiumReleaseInfoFetchError::MakingRequest)?;
    let body = resp
        .bytes()
        .await
        .map_err(ChromiumReleaseInfoFetchError::ReadingBody)?;

    let data = serde_json::from_slice(&body)?;
    Ok(data)
}

#[derive(thiserror::Error, Debug)]
pub enum UserAgentConstructionError {
    #[error("error fetching chromium releases: {0}")]
    FetchingChromiumReleases(#[from] ChromiumReleaseInfoFetchError),
    #[error("did not find applicable Chrome Stable Win32 release for version")]
    NoSuitableRelease,
}

pub async fn get_user_agent() -> Result<String, UserAgentConstructionError> {
    let releases = fetch_chromium_releases().await?;
    let target_release = match releases
        .into_iter()
        .find(|r| r.matches(TARGET_CHROMIUM_CHANNEL, TARGET_CHROMIUM_PLATFORM))
    {
        Some(release) => release,
        None => return Err(UserAgentConstructionError::NoSuitableRelease),
    };

    let version = target_release.version;

    let mut mid = BASE64_STANDARD.decode(UA_PREFIX_2).unwrap();
    // Shuffle the middle of our string to avoid presenting the same thing
    // every day.
    mid.shuffle(&mut rand::rng());

    let mut ua = String::new();
    ua.push_str(&String::from_utf8_lossy(&UA_PREFIX_1_STR));
    ua.push_str(&String::from_utf8_lossy(&mid));
    ua.push_str(&String::from_utf8_lossy(&UA_PREFIX_3_STR));
    ua.push_str(&version);
    ua.push_str(&String::from_utf8_lossy(&UA_SUFFIX_STR));

    Ok(ua)
}
