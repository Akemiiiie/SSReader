use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            details: None,
        }
    }
    pub fn storage(details: String) -> Self {
        Self {
            code: "STORAGE_UNAVAILABLE",
            message: format!("本地存储不可用：{details}"),
            retryable: true,
            details: None,
        }
    }

    pub fn migration(details: String) -> Self {
        Self {
            code: "DB_MIGRATION_FAILED",
            message: format!("数据库迁移失败，请检查本地数据目录：{details}"),
            retryable: false,
            details: None,
        }
    }
}
