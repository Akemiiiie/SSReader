use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookSummary {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub format: String,
    pub available: bool,
    pub last_opened_at: Option<String>,
    pub page_count: Option<i64>,
    pub paragraph_count: Option<i64>,
    pub txt_encoding: Option<String>,
    pub position: Option<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSelection {
    pub token: String,
    pub file_name: String,
    pub format: String,
    pub byte_size: u64,
    pub txt_encoding: Option<String>,
    pub preview: Option<String>,
    pub encoding_uncertain: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub book: BookSummary,
    pub duplicate: bool,
}
