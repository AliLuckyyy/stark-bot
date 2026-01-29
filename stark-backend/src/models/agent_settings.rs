use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Agent settings stored in database (x402 endpoint configuration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    pub id: i64,
    pub endpoint: String,
    pub model_archetype: String,
    pub max_tokens: i32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response type for agent settings API
#[derive(Debug, Clone, Serialize)]
pub struct AgentSettingsResponse {
    pub id: i64,
    pub endpoint: String,
    pub model_archetype: String,
    pub max_tokens: i32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AgentSettings> for AgentSettingsResponse {
    fn from(settings: AgentSettings) -> Self {
        Self {
            id: settings.id,
            endpoint: settings.endpoint,
            model_archetype: settings.model_archetype,
            max_tokens: settings.max_tokens,
            enabled: settings.enabled,
            created_at: settings.created_at,
            updated_at: settings.updated_at,
        }
    }
}

/// Request type for updating agent settings
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAgentSettingsRequest {
    pub endpoint: String,
    #[serde(default = "default_archetype")]
    pub model_archetype: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i32,
}

fn default_archetype() -> String {
    "kimi".to_string()
}

fn default_max_tokens() -> i32 {
    40000
}
