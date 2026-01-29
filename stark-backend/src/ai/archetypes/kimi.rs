//! Kimi Archetype - Native OpenAI-compatible tool calling
//!
//! This archetype is used for models that support native tool calling
//! through the OpenAI-compatible API (Kimi/Moonshot, OpenAI, Azure, etc.).
//!
//! Tools are passed via the API's `tools` parameter, and responses
//! contain `tool_calls` in the message structure.

use super::{AgentResponse, ArchetypeId, ModelArchetype};
use crate::tools::ToolDefinition;

/// Kimi archetype for native OpenAI-compatible tool calling
pub struct KimiArchetype;

impl KimiArchetype {
    pub fn new() -> Self {
        Self
    }
}

impl Default for KimiArchetype {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelArchetype for KimiArchetype {
    fn id(&self) -> ArchetypeId {
        ArchetypeId::Kimi
    }

    fn uses_native_tool_calling(&self) -> bool {
        true
    }

    fn default_model(&self) -> &'static str {
        "kimi-k2-turbo-preview" // Kimi K2 turbo preview - supports native tool calling per docs
    }

    fn enhance_system_prompt(&self, base_prompt: &str, tools: &[ToolDefinition]) -> String {
        if tools.is_empty() {
            return base_prompt.to_string();
        }

        let mut prompt = base_prompt.to_string();
        prompt.push_str("\n\n## Available Tools\n\n");
        prompt.push_str(
            "You have access to the following tools. Use them to help the user:\n\n",
        );

        for tool in tools {
            // Truncate long descriptions to first sentence for readability
            let short_desc = tool
                .description
                .split(". ")
                .next()
                .unwrap_or(&tool.description);
            prompt.push_str(&format!("- **{}**: {}\n", tool.name, short_desc));
        }

        prompt.push_str("\n**IMPORTANT**: When a user asks for something that a tool can provide, ");
        prompt.push_str("USE the tool via the native tool_calls mechanism. ");
        prompt.push_str("Do not output tool calls as text.\n");

        prompt
    }

    fn parse_response(&self, content: &str) -> Option<AgentResponse> {
        // Native tool calling uses the API's tool_calls field, not text parsing
        // This is only called if there's text content without tool calls
        Some(AgentResponse {
            body: content.to_string(),
            tool_call: None,
        })
    }

    fn format_tool_followup(&self, _tool_name: &str, _tool_result: &str, _success: bool) -> String {
        // Native tool calling uses the API's message format for tool results
        // This shouldn't be called for native archetypes, but provide a fallback
        String::new()
    }
}
