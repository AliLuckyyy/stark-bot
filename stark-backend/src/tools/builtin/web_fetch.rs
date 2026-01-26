use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Deserialize a usize from either a number or a string
fn deserialize_usize_lenient<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<Value> = Option::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(Value::Number(n)) => Ok(n.as_u64().map(|v| v as usize)),
        Some(Value::String(s)) => Ok(s.parse().ok()),
        _ => Ok(None),
    }
}

/// Cache entry for fetch results
struct CacheEntry {
    result: ToolResult,
    expires_at: Instant,
}

/// Simple in-memory cache with TTL
struct FetchCache {
    entries: RwLock<HashMap<String, CacheEntry>>,
    ttl: Duration,
}

impl FetchCache {
    fn new(ttl_secs: u64) -> Self {
        FetchCache {
            entries: RwLock::new(HashMap::new()),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    fn get(&self, key: &str) -> Option<ToolResult> {
        let entries = self.entries.read().ok()?;
        if let Some(entry) = entries.get(key) {
            if entry.expires_at > Instant::now() {
                return Some(entry.result.clone());
            }
        }
        None
    }

    fn set(&self, key: String, result: ToolResult) {
        if let Ok(mut entries) = self.entries.write() {
            // Clean expired entries occasionally
            if entries.len() > 50 {
                let now = Instant::now();
                entries.retain(|_, v| v.expires_at > now);
            }
            entries.insert(
                key,
                CacheEntry {
                    result,
                    expires_at: Instant::now() + self.ttl,
                },
            );
        }
    }
}

/// Web fetch tool to retrieve and parse content from URLs
pub struct WebFetchTool {
    definition: ToolDefinition,
    cache: FetchCache,
}

impl WebFetchTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();
        properties.insert(
            "url".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "The URL to fetch content from (HTTP/HTTPS only)".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );
        properties.insert(
            "extract_mode".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Output format: 'markdown' for readable markdown, 'text' for plain text, 'raw' for unprocessed content".to_string(),
                default: Some(json!("markdown")),
                items: None,
                enum_values: Some(vec![
                    "markdown".to_string(),
                    "text".to_string(),
                    "raw".to_string(),
                ]),
            },
        );
        properties.insert(
            "max_chars".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Maximum content length to return (default: 50000 characters)"
                    .to_string(),
                default: Some(json!(50000)),
                items: None,
                enum_values: None,
            },
        );

        WebFetchTool {
            definition: ToolDefinition {
                name: "web_fetch".to_string(),
                description: "Fetch content from a URL and extract readable text or markdown. Blocks private/internal URLs for security.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["url".to_string()],
                },
                group: ToolGroup::Web,
            },
            cache: FetchCache::new(900), // 15 minute cache
        }
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct WebFetchParams {
    url: String,
    #[serde(alias = "max_length", default, deserialize_with = "deserialize_usize_lenient")]
    max_chars: Option<usize>,
    extract_mode: Option<String>,
    // Legacy parameter support
    extract_text: Option<bool>,
}

#[async_trait]
impl Tool for WebFetchTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        let params: WebFetchParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        let max_chars = params.max_chars.unwrap_or(50000);

        // Handle extract_mode with legacy extract_text fallback
        let extract_mode = params.extract_mode.unwrap_or_else(|| {
            if params.extract_text == Some(false) {
                "raw".to_string()
            } else {
                "markdown".to_string()
            }
        });

        // Validate URL scheme
        if !params.url.starts_with("http://") && !params.url.starts_with("https://") {
            return ToolResult::error("URL must start with http:// or https://");
        }

        // Parse and validate URL
        let url = match url::Url::parse(&params.url) {
            Ok(u) => u,
            Err(e) => return ToolResult::error(format!("Invalid URL: {}", e)),
        };

        // Check for private/internal hostnames
        if let Err(e) = validate_public_url(&url) {
            return ToolResult::error(e);
        }

        // Build cache key
        let cache_key = format!("{}:{}:{}", params.url, extract_mode, max_chars);

        // Check cache first
        if let Some(cached) = self.cache.get(&cache_key) {
            log::debug!("web_fetch: returning cached result for URL '{}'", params.url);
            return cached;
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("StarkBot/1.0 (Web Fetch Tool)")
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let response = match client.get(&params.url).send().await {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Failed to fetch URL: {}", e)),
        };

        let final_url = response.url().to_string();
        let status = response.status();

        if !status.is_success() {
            return ToolResult::error(format!("HTTP error: {} for URL: {}", status, params.url));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let body = match response.text().await {
            Ok(t) => t,
            Err(e) => return ToolResult::error(format!("Failed to read response body: {}", e)),
        };

        let original_length = body.len();
        let is_html = content_type.contains("text/html");

        let content = match extract_mode.as_str() {
            "raw" => body,
            "text" if is_html => extract_text_from_html(&body),
            "markdown" if is_html => extract_markdown_from_html(&body),
            _ => body, // For non-HTML, return as-is
        };

        // Truncate if necessary
        let truncated = content.len() > max_chars;
        let final_content = if truncated {
            format!(
                "{}\n\n[Content truncated at {} characters. Original length: {} characters]",
                &content[..max_chars],
                max_chars,
                content.len()
            )
        } else {
            content
        };

        let result = ToolResult::success(final_content).with_metadata(json!({
            "url": params.url,
            "final_url": final_url,
            "content_type": content_type,
            "extract_mode": extract_mode,
            "truncated": truncated,
            "original_length": original_length,
            "cached": false
        }));

        // Cache successful results
        self.cache.set(cache_key, result.clone());

        result
    }
}

/// Validate that a URL points to a public host (not private/internal)
fn validate_public_url(url: &url::Url) -> Result<(), String> {
    let host = url.host_str().ok_or("URL has no host")?;

    // Block localhost and common internal hostnames
    let blocked_hosts = [
        "localhost",
        "127.0.0.1",
        "0.0.0.0",
        "::1",
        "[::1]",
        "metadata.google.internal",
        "metadata.google",
        "169.254.169.254", // AWS/GCP metadata
    ];

    let host_lower = host.to_lowercase();
    if blocked_hosts.contains(&host_lower.as_str()) {
        return Err(format!("Access to internal host '{}' is blocked", host));
    }

    // Block .local, .internal, .localhost TLDs
    if host_lower.ends_with(".local")
        || host_lower.ends_with(".internal")
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".lan")
    {
        return Err(format!("Access to internal domain '{}' is blocked", host));
    }

    // Try to resolve and check if it's a private IP
    let port = url.port().unwrap_or(if url.scheme() == "https" { 443 } else { 80 });
    if let Ok(addrs) = format!("{}:{}", host, port).to_socket_addrs() {
        for addr in addrs {
            if is_private_ip(addr.ip()) {
                return Err(format!(
                    "URL resolves to private IP address '{}', access blocked",
                    addr.ip()
                ));
            }
        }
    }

    Ok(())
}

/// Check if an IP address is private/internal
fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_private()           // 10.x, 172.16-31.x, 192.168.x
                || ipv4.is_loopback()   // 127.x
                || ipv4.is_link_local() // 169.254.x
                || ipv4.is_broadcast()
                || ipv4.is_documentation()
                || ipv4.is_unspecified()
                // Cloud metadata IPs
                || ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback() || ipv6.is_unspecified()
            // Note: is_unique_local() and is_unicast_link_local() are unstable
        }
    }
}

/// Extract readable markdown from HTML
fn extract_markdown_from_html(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut current_tag = String::new();
    let mut tag_stack: Vec<String> = Vec::new();
    let mut last_was_block = false;

    let html_lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let chars_lower: Vec<char> = html_lower.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        // Check for script/style tags
        if i + 7 < chars_lower.len() {
            let slice: String = chars_lower[i..i + 7].iter().collect();
            if slice == "<script" {
                in_script = true;
            }
            if slice == "</scrip" {
                in_script = false;
            }
        }
        if i + 6 < chars_lower.len() {
            let slice: String = chars_lower[i..i + 6].iter().collect();
            if slice == "<style" {
                in_style = true;
            }
            if slice == "</styl" {
                in_style = false;
            }
        }

        if c == '<' {
            in_tag = true;
            current_tag.clear();
            i += 1;
            continue;
        }

        if c == '>' {
            in_tag = false;
            let tag_lower = current_tag.to_lowercase();
            let tag_name = tag_lower.split_whitespace().next().unwrap_or("");
            let is_closing = tag_name.starts_with('/');
            let base_tag = tag_name.trim_start_matches('/');

            // Handle markdown formatting based on tags
            match base_tag {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    if !is_closing {
                        if !result.ends_with('\n') && !result.is_empty() {
                            result.push_str("\n\n");
                        }
                        let level = base_tag.chars().last().unwrap().to_digit(10).unwrap_or(1);
                        result.push_str(&"#".repeat(level as usize));
                        result.push(' ');
                        tag_stack.push(base_tag.to_string());
                    } else {
                        tag_stack.pop();
                        result.push_str("\n\n");
                        last_was_block = true;
                    }
                }
                "p" | "div" | "article" | "section" => {
                    if is_closing && !last_was_block {
                        result.push_str("\n\n");
                        last_was_block = true;
                    } else if !is_closing {
                        last_was_block = false;
                    }
                }
                "br" => {
                    result.push('\n');
                }
                "li" => {
                    if !is_closing {
                        if !result.ends_with('\n') {
                            result.push('\n');
                        }
                        result.push_str("- ");
                    } else {
                        result.push('\n');
                    }
                }
                "strong" | "b" => {
                    result.push_str("**");
                }
                "em" | "i" => {
                    result.push('*');
                }
                "code" => {
                    result.push('`');
                }
                "pre" => {
                    if !is_closing {
                        result.push_str("\n```\n");
                    } else {
                        result.push_str("\n```\n");
                    }
                }
                "a" => {
                    if !is_closing {
                        // Extract href
                        if let Some(href_start) = tag_lower.find("href=\"") {
                            let href_content = &current_tag[href_start + 6..];
                            if let Some(href_end) = href_content.find('"') {
                                let href = &href_content[..href_end];
                                tag_stack.push(format!("a:{}", href));
                            }
                        }
                        result.push('[');
                    } else {
                        result.push(']');
                        // Find matching opening tag with href
                        if let Some(pos) = tag_stack.iter().rposition(|t| t.starts_with("a:")) {
                            let href = tag_stack[pos].strip_prefix("a:").unwrap_or("");
                            result.push_str(&format!("({})", href));
                            tag_stack.remove(pos);
                        }
                    }
                }
                "blockquote" => {
                    if !is_closing {
                        result.push_str("\n> ");
                    } else {
                        result.push('\n');
                    }
                }
                "hr" => {
                    result.push_str("\n---\n");
                }
                _ => {}
            }
            current_tag.clear();
            i += 1;
            continue;
        }

        if in_tag {
            current_tag.push(c);
            i += 1;
            continue;
        }

        if !in_script && !in_style {
            // Handle HTML entities
            if c == '&' {
                let remaining: String = chars[i..].iter().take(10).collect();
                if remaining.starts_with("&nbsp;") {
                    result.push(' ');
                    i += 6;
                    continue;
                } else if remaining.starts_with("&amp;") {
                    result.push('&');
                    i += 5;
                    continue;
                } else if remaining.starts_with("&lt;") {
                    result.push('<');
                    i += 4;
                    continue;
                } else if remaining.starts_with("&gt;") {
                    result.push('>');
                    i += 4;
                    continue;
                } else if remaining.starts_with("&quot;") {
                    result.push('"');
                    i += 6;
                    continue;
                } else if remaining.starts_with("&apos;") {
                    result.push('\'');
                    i += 6;
                    continue;
                } else if remaining.starts_with("&#") {
                    if let Some(end) = remaining.find(';') {
                        let code_str = &remaining[2..end];
                        let code = if code_str.starts_with('x') || code_str.starts_with('X') {
                            u32::from_str_radix(&code_str[1..], 16).ok()
                        } else {
                            code_str.parse::<u32>().ok()
                        };
                        if let Some(code) = code {
                            if let Some(ch) = char::from_u32(code) {
                                result.push(ch);
                                i += end + 1;
                                continue;
                            }
                        }
                    }
                }
            }

            result.push(c);
            if !c.is_whitespace() {
                last_was_block = false;
            }
        }

        i += 1;
    }

    // Clean up the result
    clean_text(&result)
}

/// Extract plain text from HTML (simpler extraction)
fn extract_text_from_html(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut last_was_space = false;

    let html_lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let chars_lower: Vec<char> = html_lower.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        // Check for script/style tags
        if i + 7 < chars_lower.len() {
            let slice: String = chars_lower[i..i + 7].iter().collect();
            if slice == "<script" {
                in_script = true;
            }
            if slice == "</scrip" {
                in_script = false;
            }
        }
        if i + 6 < chars_lower.len() {
            let slice: String = chars_lower[i..i + 6].iter().collect();
            if slice == "<style" {
                in_style = true;
            }
            if slice == "</styl" {
                in_style = false;
            }
        }

        if c == '<' {
            in_tag = true;
            i += 1;
            continue;
        }

        if c == '>' {
            in_tag = false;
            // Add newline after certain tags
            if i >= 3 {
                let prev: String = chars_lower[i.saturating_sub(3)..i].iter().collect();
                if prev.contains("/p")
                    || prev.contains("br")
                    || prev.contains("/h")
                    || prev.contains("/li")
                    || prev.contains("/tr")
                    || prev.contains("/di")
                {
                    if !last_was_space {
                        text.push('\n');
                        last_was_space = true;
                    }
                }
            }
            i += 1;
            continue;
        }

        if !in_tag && !in_script && !in_style {
            // Handle HTML entities
            if c == '&' {
                let remaining: String = chars[i..].iter().take(10).collect();
                if remaining.starts_with("&nbsp;") {
                    text.push(' ');
                    i += 6;
                    continue;
                } else if remaining.starts_with("&amp;") {
                    text.push('&');
                    i += 5;
                    continue;
                } else if remaining.starts_with("&lt;") {
                    text.push('<');
                    i += 4;
                    continue;
                } else if remaining.starts_with("&gt;") {
                    text.push('>');
                    i += 4;
                    continue;
                } else if remaining.starts_with("&quot;") {
                    text.push('"');
                    i += 6;
                    continue;
                } else if remaining.starts_with("&#") {
                    // Numeric entity
                    if let Some(end) = remaining.find(';') {
                        if let Ok(code) = remaining[2..end].parse::<u32>() {
                            if let Some(ch) = char::from_u32(code) {
                                text.push(ch);
                                i += end + 1;
                                continue;
                            }
                        }
                    }
                }
            }

            // Normalize whitespace
            if c.is_whitespace() {
                if !last_was_space {
                    text.push(' ');
                    last_was_space = true;
                }
            } else {
                text.push(c);
                last_was_space = false;
            }
        }

        i += 1;
    }

    clean_text(&text)
}

/// Clean up extracted text
fn clean_text(text: &str) -> String {
    text.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        // Collapse multiple newlines
        .split("\n\n\n")
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_from_html() {
        let html = r#"
        <html>
        <head><title>Test</title></head>
        <body>
            <h1>Hello World</h1>
            <p>This is a <b>test</b> paragraph.</p>
            <script>var x = 1;</script>
            <p>Second paragraph with &amp; entity.</p>
        </body>
        </html>
        "#;

        let text = extract_text_from_html(html);
        assert!(text.contains("Hello World"));
        assert!(text.contains("This is a test paragraph."));
        assert!(text.contains("&"));
        assert!(!text.contains("var x = 1"));
    }

    #[test]
    fn test_extract_markdown_from_html() {
        let html = r#"
        <h1>Main Title</h1>
        <p>This is <strong>bold</strong> and <em>italic</em>.</p>
        <ul>
            <li>Item 1</li>
            <li>Item 2</li>
        </ul>
        "#;

        let md = extract_markdown_from_html(html);
        assert!(md.contains("# Main Title"));
        assert!(md.contains("**bold**"));
        assert!(md.contains("*italic*"));
        assert!(md.contains("- Item 1"));
    }

    #[test]
    fn test_private_ip_detection() {
        assert!(is_private_ip("127.0.0.1".parse().unwrap()));
        assert!(is_private_ip("192.168.1.1".parse().unwrap()));
        assert!(is_private_ip("10.0.0.1".parse().unwrap()));
        assert!(is_private_ip("172.16.0.1".parse().unwrap()));
        assert!(!is_private_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip("1.1.1.1".parse().unwrap()));
    }
}
