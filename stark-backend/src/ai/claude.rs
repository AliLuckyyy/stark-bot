use crate::ai::{Message, MessageRole};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ClaudeClient {
    client: Client,
    model: String,
}

#[derive(Debug, Serialize)]
struct ClaudeCompletionRequest {
    model: String,
    messages: Vec<ClaudeMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Debug, Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeCompletionResponse {
    content: Vec<ClaudeResponseContent>,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponseContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeErrorResponse {
    error: ClaudeError,
}

#[derive(Debug, Deserialize)]
struct ClaudeError {
    message: String,
}

impl ClaudeClient {
    pub fn new(api_key: &str, model: Option<&str>) -> Result<Self, String> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );

        let auth_value = header::HeaderValue::from_str(api_key)
            .map_err(|e| format!("Invalid API key format: {}", e))?;
        headers.insert("x-api-key", auth_value);
        headers.insert(
            "anthropic-version",
            header::HeaderValue::from_static("2023-06-01"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            client,
            model: model.unwrap_or("claude-sonnet-4-20250514").to_string(),
        })
    }

    pub async fn generate_text(&self, messages: Vec<Message>) -> Result<String, String> {
        // Extract system message if present
        let mut system_message = None;
        let filtered_messages: Vec<Message> = messages
            .into_iter()
            .filter(|m| {
                if m.role == MessageRole::System {
                    system_message = Some(m.content.clone());
                    false
                } else {
                    true
                }
            })
            .collect();

        let api_messages: Vec<ClaudeMessage> = filtered_messages
            .into_iter()
            .map(|m| ClaudeMessage {
                role: m.role.to_string(),
                content: m.content,
            })
            .collect();

        let request = ClaudeCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            max_tokens: 4096,
            system: system_message,
        };

        log::debug!("Sending request to Claude API: {:?}", request);

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Claude API request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

            // Try to parse the error response
            if let Ok(error_response) = serde_json::from_str::<ClaudeErrorResponse>(&error_text) {
                return Err(format!("Claude API error: {}", error_response.error.message));
            }

            return Err(format!(
                "Claude API returned error status: {}, body: {}",
                status, error_text
            ));
        }

        let response_data: ClaudeCompletionResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Claude response: {}", e))?;

        // Concatenate all text content from response
        let content: String = response_data
            .content
            .iter()
            .filter(|c| c.content_type == "text")
            .filter_map(|c| c.text.clone())
            .collect();

        if content.is_empty() {
            return Err("Claude API returned no content".to_string());
        }

        Ok(content)
    }
}
