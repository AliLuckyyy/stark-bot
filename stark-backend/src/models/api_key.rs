use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: i64,
    pub service_name: String,
    #[serde(skip_serializing)]
    pub api_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response version with masked key
#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyResponse {
    pub id: i64,
    pub service_name: String,
    pub key_preview: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ApiKey> for ApiKeyResponse {
    fn from(key: ApiKey) -> Self {
        // Show first 4 and last 4 characters, mask the rest
        let key_preview = if key.api_key.len() > 12 {
            let start = &key.api_key[..4];
            let end = &key.api_key[key.api_key.len() - 4..];
            format!("{}...{}", start, end)
        } else {
            "****".to_string()
        };

        Self {
            id: key.id,
            service_name: key.service_name,
            key_preview,
            created_at: key.created_at,
            updated_at: key.updated_at,
        }
    }
}
