use std::ops::Deref;
// use std::collections::HashMap;
use std::path::Path;

use chrono::NaiveDate;
use google_sheets4::api::{
    BatchUpdateSpreadsheetRequest, BatchUpdateValuesRequest, DuplicateSheetRequest, Request,
    ValueRange,
};
use google_sheets4::hyper::client::HttpConnector;
use google_sheets4::hyper_rustls::HttpsConnector;
use google_sheets4::{hyper, hyper_rustls, oauth2, Sheets};
use serde_json::json;

use crate::{LengthInfo, PairInfo};

#[derive(Debug, thiserror::Error)]
pub enum NewSheetError {
    #[error("failed to read service account credentials file: {0}")]
    ReadingCredentialsFile(std::io::Error),
    #[error("failed to authenticate as service account: {0}")]
    AuthenticatingAsServiceAccount(std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum FindingTemplateError {
    #[error("error reaching Sheets API: {0}")]
    APIError(#[from] google_sheets4::Error),
    #[error("no sheets in get() response")]
    NoSheets,
    #[error("did not find template sheet")]
    DidNotFindSheet,
}

#[derive(Debug, thiserror::Error)]
pub enum DuplicatingTemplateError {
    #[error("API request failed: {0}")]
    RequestFailed(#[from] google_sheets4::Error),
    #[error("Response missing key fields")]
    MissingResponse,
}

#[derive(Debug, thiserror::Error)]
pub enum PopulateNewSheetError {
    #[error("API request failed: {0}")]
    RequestFailed(#[from] google_sheets4::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SheetCreationError {
    #[error("could not identify template sheet: {0}")]
    IdentifyingTemplateSheet(#[from] FindingTemplateError),
    #[error("could not duplicate template sheet: {0}")]
    DuplicatingTemplateSheet(#[from] DuplicatingTemplateError),
    #[error("could not populate data in new sheet: {0}")]
    PopulatingNewSheet(#[from] PopulateNewSheetError),
}

fn pairs_to_values(pairs: &PairInfo) -> Vec<Vec<serde_json::Value>> {
    pairs
        .iter()
        .map(|((a, b), v)| vec![json!(format!("{a}{b}")), json!(v)])
        .collect()
}

fn lengths_to_values(lengths: &LengthInfo) -> Vec<Vec<serde_json::Value>> {
    lengths
        .iter()
        .map(|((letter, len), quantity)| vec![json!(letter), json!(len), json!(quantity)])
        .collect()
}

pub struct SheetManager {
    client: Sheets<HttpsConnector<HttpConnector>>,
    spreadsheet_id: String,
}

fn is_template(sheet: &google_sheets4::api::Sheet) -> bool {
    sheet
        .properties
        .as_ref()
        .and_then(|props| props.title.as_ref())
        .map(|title| title == "TEMPLATE")
        .unwrap_or(false)
}

impl SheetManager {
    pub async fn new<P, S>(
        spreadsheet_id: S,
        service_account_file: P,
    ) -> Result<Self, NewSheetError>
    where
        P: AsRef<Path>,
        S: Deref<Target = String>,
    {
        let creds = google_sheets4::oauth2::read_service_account_key(service_account_file)
            .await
            .map_err(NewSheetError::ReadingCredentialsFile)?;
        let auth = oauth2::ServiceAccountAuthenticator::builder(creds)
            .build()
            .await
            .map_err(NewSheetError::AuthenticatingAsServiceAccount)?;
        let http_client = hyper::Client::builder().build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .unwrap()
                .https_only()
                .enable_http2()
                .build(),
        );
        Ok(Self {
            client: Sheets::new(http_client, auth),
            spreadsheet_id: spreadsheet_id.to_string(),
        })
    }

    pub async fn create_for_date(
        &self,
        date: &NaiveDate,
        pairs: &PairInfo,
        lengths: &LengthInfo,
    ) -> Result<(), SheetCreationError> {
        let template_sheet = self.find_template().await?;
        let template_sheet_id = template_sheet
            .properties
            .and_then(|p| p.sheet_id)
            .expect("missing sheet ID");
        let new_sheet = self.duplicate_template(date, template_sheet_id).await?;
        let new_sheet_name = new_sheet.title.expect("missing name of new sheet");
        self.populate_new_sheet(&new_sheet_name, pairs, lengths)
            .await?;
        Ok(())
    }

    async fn find_template(&self) -> Result<google_sheets4::api::Sheet, FindingTemplateError> {
        self.client
            .spreadsheets()
            .get(&self.spreadsheet_id)
            // perform spreadsheets.get() request
            .doit()
            .await?
            // get parsed data only
            .1
            // sheets of document
            .sheets
            .ok_or(FindingTemplateError::NoSheets)?
            .into_iter()
            // find template sheet in sheets
            .find(is_template)
            .ok_or(FindingTemplateError::DidNotFindSheet)
    }

    async fn duplicate_template(
        &self,
        date: &NaiveDate,
        template_id: i32,
    ) -> Result<google_sheets4::api::SheetProperties, DuplicatingTemplateError> {
        let duplicate_req = DuplicateSheetRequest {
            source_sheet_id: Some(template_id),
            insert_sheet_index: Some(1),
            new_sheet_name: Some(date.format("%Y-%m-%d").to_string()),
            new_sheet_id: None,
        };
        let request = BatchUpdateSpreadsheetRequest {
            requests: Some(vec![Request {
                duplicate_sheet: Some(duplicate_req),
                ..Default::default()
            }]),
            ..Default::default()
        };

        self.client
            .spreadsheets()
            .batch_update(request, &self.spreadsheet_id)
            .doit()
            .await?
            // parsed response only
            .1
            .replies
            // use mut vector out so we can remove only response
            .as_mut()
            .map(|replies| replies.remove(0))
            .and_then(|reply| reply.duplicate_sheet)
            .and_then(|resp| resp.properties)
            .ok_or(DuplicatingTemplateError::MissingResponse)
    }

    async fn populate_new_sheet(
        &self,
        sheet_name: &str,
        pairs: &PairInfo,
        lengths: &LengthInfo,
    ) -> Result<(), PopulateNewSheetError> {
        let pairs_value_range = ValueRange {
            major_dimension: Some("ROWS".to_string()),
            range: Some(format!("'{sheet_name}'!F3:G")),
            values: Some(pairs_to_values(pairs)),
        };

        let lengths_value_range = ValueRange {
            major_dimension: Some("ROWS".to_string()),
            range: Some(format!("'{sheet_name}'!B3:D")),
            values: Some(lengths_to_values(lengths)),
        };

        let request = BatchUpdateValuesRequest {
            data: Some(vec![pairs_value_range, lengths_value_range]),
            value_input_option: Some("RAW".to_string()),
            ..Default::default()
        };

        self.client
            .spreadsheets()
            .values_batch_update(request, &self.spreadsheet_id)
            .doit()
            .await?;

        Ok(())
    }
}
