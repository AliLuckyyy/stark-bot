use crate::ai::types::{AiResponse, ToolCall};
use crate::ai::Message;
use crate::tools::ToolDefinition;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

/// Llama client for Ollama API (with tool support for Llama 3.1+)
#[derive(Debug, Clone)]
pub struct LlamaClient {
    client: Client,
    endpoint: String,
    model: String,
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaTool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Debug, Serialize, Clone)]
struct OllamaTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OllamaToolFunction,
}

#[derive(Debug, Serialize, Clone)]
struct OllamaToolFunction {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OllamaToolCall {
    #[serde(default)]
    pub id: Option<String>,
    pub function: OllamaFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OllamaFunctionCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
    #[serde(default)]
    done_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OllamaErrorResponse {
    error: String,
}

impl LlamaClient {
    pub fn new(endpoint: Option<&str>, model: Option<&str>) -> Result<Self, String> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(300)) // Llama can be slower
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            client,
            endpoint: endpoint
                .unwrap_or("http://localhost:11434/api/chat")
                .to_string(),
            model: model.unwrap_or("llama3.3").to_string(),
        })
    }

    pub async fn generate_text(&self, messages: Vec<Message>) -> Result<String, String> {
        let api_messages: Vec<OllamaMessage> = messages
            .into_iter()
            .map(|m| OllamaMessage {
                role: m.role.to_string(),
                content: m.content,
                tool_calls: None,
            })
            .collect();

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: api_messages,
            stream: false,
            tools: None,
        };

        log::debug!("Sending request to Ollama API: {:?}", request);

        let response = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Ollama API request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

            if let Ok(error_response) = serde_json::from_str::<OllamaErrorResponse>(&error_text) {
                return Err(format!("Ollama API error: {}", error_response.error));
            }

            return Err(format!(
                "Ollama API returned error status: {}, body: {}",
                status, error_text
            ));
        }

        let response_data: OllamaChatResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        if response_data.message.content.is_empty() {
            return Err("Ollama API returned no content".to_string());
        }

        Ok(response_data.message.content)
    }

    /// Generate a response with tool support (Llama 3.1+ with Ollama)
    pub async fn generate_with_tools(
        &self,
        messages: Vec<Message>,
        tool_messages: Vec<OllamaMessage>,
        tools: Vec<ToolDefinition>,
    ) -> Result<AiResponse, String> {
        // Convert messages to Ollama format
        let mut api_messages: Vec<OllamaMessage> = messages
            .into_iter()
            .map(|m| OllamaMessage {
                role: m.role.to_string(),
                content: m.content,
                tool_calls: None,
            })
            .collect();

        // Add tool conversation history
        api_messages.extend(tool_messages);

        // Convert tool definitions to Ollama format
        let ollama_tools: Vec<OllamaTool> = tools
            .into_iter()
            .map(|t| OllamaTool {
                tool_type: "function".to_string(),
                function: OllamaToolFunction {
                    name: t.name,
                    description: t.description,
                    parameters: serde_json::to_value(t.input_schema).unwrap_or_default(),
                },
            })
            .collect();

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: api_messages,
            stream: false,
            tools: if ollama_tools.is_empty() {
                None
            } else {
                Some(ollama_tools)
            },
        };

        log::debug!(
            "Sending tool request to Ollama API: {}",
            serde_json::to_string_pretty(&request).unwrap_or_default()
        );

        let response = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Ollama API request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

            if let Ok(error_response) = serde_json::from_str::<OllamaErrorResponse>(&error_text) {
                return Err(format!("Ollama API error: {}", error_response.error));
            }

            return Err(format!(
                "Ollama API returned error status: {}, body: {}",
                status, error_text
            ));
        }

        let response_data: OllamaChatResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        // Parse tool calls from response
        let mut tool_calls = Vec::new();
        if let Some(calls) = response_data.message.tool_calls {
            for (idx, call) in calls.into_iter().enumerate() {
                tool_calls.push(ToolCall {
                    id: call.id.unwrap_or_else(|| format!("call_{}", idx)),
                    name: call.function.name,
                    arguments: call.function.arguments,
                });
            }
        }

        // Determine stop reason
        let stop_reason = if !tool_calls.is_empty() {
            Some("tool_use".to_string())
        } else {
            response_data.done_reason
        };

        Ok(AiResponse {
            content: response_data.message.content,
            tool_calls,
            stop_reason,
        })
    }

    /// Build tool result messages for continuing conversation after tool execution
    pub fn build_tool_result_messages(
        tool_calls: &[ToolCall],
        tool_responses: &[crate::ai::ToolResponse],
    ) -> Vec<OllamaMessage> {
        let mut messages = Vec::new();

        // Assistant message with tool calls
        let ollama_tool_calls: Vec<OllamaToolCall> = tool_calls
            .iter()
            .map(|tc| OllamaToolCall {
                id: Some(tc.id.clone()),
                function: OllamaFunctionCall {
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                },
            })
            .collect();

        messages.push(OllamaMessage {
            role: "assistant".to_string(),
            content: String::new(),
            tool_calls: Some(ollama_tool_calls),
        });

        // Tool response messages
        for response in tool_responses {
            messages.push(OllamaMessage {
                role: "tool".to_string(),
                content: response.content.clone(),
                tool_calls: None,
            });
        }

        messages
    }
}

/// Re-export for use in AiClient
pub use OllamaMessage as LlamaMessage;
