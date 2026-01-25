use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::ai::{ClaudeClient, Message, MessageRole};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/api/chat").route(web::post().to(chat)));
}

async fn chat(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<ChatRequest>,
) -> impl Responder {
    // Validate session token
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim_start_matches("Bearer ").to_string());

    let token = match token {
        Some(t) => t,
        None => {
            return HttpResponse::Unauthorized().json(ChatResponse {
                success: false,
                message: None,
                error: Some("No authorization token provided".to_string()),
            });
        }
    };

    // Validate the session
    match state.db.validate_session(&token) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::Unauthorized().json(ChatResponse {
                success: false,
                message: None,
                error: Some("Invalid or expired session".to_string()),
            });
        }
        Err(e) => {
            log::error!("Failed to validate session: {}", e);
            return HttpResponse::InternalServerError().json(ChatResponse {
                success: false,
                message: None,
                error: Some("Internal server error".to_string()),
            });
        }
    }

    // Get Anthropic API key from database
    let api_key = match state.db.get_api_key("anthropic") {
        Ok(Some(key)) => key.api_key,
        Ok(None) => {
            return HttpResponse::ServiceUnavailable().json(ChatResponse {
                success: false,
                message: None,
                error: Some("Anthropic API key not configured. Please add it in API Keys settings.".to_string()),
            });
        }
        Err(e) => {
            log::error!("Failed to get API key: {}", e);
            return HttpResponse::InternalServerError().json(ChatResponse {
                success: false,
                message: None,
                error: Some("Failed to retrieve API configuration".to_string()),
            });
        }
    };

    // Create Claude client
    let claude_client = match ClaudeClient::new(&api_key, None) {
        Ok(client) => client,
        Err(e) => {
            log::error!("Failed to create Claude client: {}", e);
            return HttpResponse::InternalServerError().json(ChatResponse {
                success: false,
                message: None,
                error: Some("Failed to initialize AI service".to_string()),
            });
        }
    };

    // Convert messages to AI format
    let ai_messages: Vec<Message> = body
        .messages
        .iter()
        .map(|m| Message {
            role: match m.role.as_str() {
                "system" => MessageRole::System,
                "assistant" => MessageRole::Assistant,
                _ => MessageRole::User,
            },
            content: m.content.clone(),
        })
        .collect();

    // Add system message if not present
    let has_system = ai_messages.iter().any(|m| m.role == MessageRole::System);
    let messages = if has_system {
        ai_messages
    } else {
        let mut msgs = vec![Message {
            role: MessageRole::System,
            content: "You are StarkBot, a helpful AI assistant. Be concise and helpful.".to_string(),
        }];
        msgs.extend(ai_messages);
        msgs
    };

    // Call Claude API
    match claude_client.generate_text(messages).await {
        Ok(response_text) => HttpResponse::Ok().json(ChatResponse {
            success: true,
            message: Some(ChatMessage {
                role: "assistant".to_string(),
                content: response_text,
            }),
            error: None,
        }),
        Err(e) => {
            log::error!("Claude API error: {}", e);
            HttpResponse::InternalServerError().json(ChatResponse {
                success: false,
                message: None,
                error: Some(format!("AI service error: {}", e)),
            })
        }
    }
}
