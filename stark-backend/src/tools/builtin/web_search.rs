use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Deserialize a u32 from either a number or a string
fn deserialize_u32_lenient<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<Value> = Option::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(Value::Number(n)) => Ok(n.as_u64().map(|v| v as u32)),
        Some(Value::String(s)) => Ok(s.parse().ok()),
        _ => Ok(None),
    }
}

/// Cache entry for search results
struct CacheEntry {
    result: ToolResult,
    expires_at: Instant,
}

/// Simple in-memory cache with TTL
struct SearchCache {
    entries: RwLock<HashMap<String, CacheEntry>>,
    ttl: Duration,
}

impl SearchCache {
    fn new(ttl_secs: u64) -> Self {
        SearchCache {
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
            if entries.len() > 100 {
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

/// Web search tool using search APIs (Brave, SerpAPI, etc.)
pub struct WebSearchTool {
    definition: ToolDefinition,
    cache: SearchCache,
}

impl WebSearchTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();
        properties.insert(
            "query".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "The search query".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );
        properties.insert(
            "count".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Number of results to return (1-10, default: 5)".to_string(),
                default: Some(json!(5)),
                items: None,
                enum_values: None,
            },
        );
        properties.insert(
            "country".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Two-letter country code for regional filtering (e.g., 'us', 'gb', 'de')".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );
        properties.insert(
            "freshness".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Time filter: 'day' (past 24h), 'week', 'month', 'year', or date range 'YYYY-MM-DD:YYYY-MM-DD'".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "day".to_string(),
                    "week".to_string(),
                    "month".to_string(),
                    "year".to_string(),
                ]),
            },
        );
        properties.insert(
            "search_lang".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Language code for search results (e.g., 'en', 'es', 'fr')".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        WebSearchTool {
            definition: ToolDefinition {
                name: "web_search".to_string(),
                description: "Search the web for information. Returns a list of relevant web pages with titles, URLs, and snippets. Supports filtering by country, language, and time freshness.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["query".to_string()],
                },
                group: ToolGroup::Web,
            },
            cache: SearchCache::new(900), // 15 minute cache
        }
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct WebSearchParams {
    query: String,
    #[serde(alias = "num_results", default, deserialize_with = "deserialize_u32_lenient")]
    count: Option<u32>,
    country: Option<String>,
    freshness: Option<String>,
    search_lang: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

// Brave Search API response structures
#[derive(Debug, Deserialize)]
struct BraveSearchResponse {
    web: Option<BraveWebResults>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResults {
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    description: String,
}

// SerpAPI response structures
#[derive(Debug, Deserialize)]
struct SerpApiResponse {
    organic_results: Option<Vec<SerpResult>>,
}

#[derive(Debug, Deserialize)]
struct SerpResult {
    title: String,
    link: String,
    snippet: Option<String>,
}

#[async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        let params: WebSearchParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        let count = params.count.unwrap_or(5).min(10).max(1);

        // Build cache key
        let cache_key = format!(
            "{}:{}:{}:{}:{}",
            params.query,
            count,
            params.country.as_deref().unwrap_or(""),
            params.freshness.as_deref().unwrap_or(""),
            params.search_lang.as_deref().unwrap_or("")
        );

        // Check cache first
        if let Some(cached) = self.cache.get(&cache_key) {
            log::debug!("web_search: returning cached result for query '{}'", params.query);
            return cached;
        }

        // Try different search API providers
        // First check context (database-stored keys), then fall back to env vars

        // Check for Brave Search API key (context first, then env)
        if let Some(api_key) = context.get_api_key("brave_search") {
            let result = self.search_brave(&params, count, &api_key).await;
            if result.success {
                self.cache.set(cache_key, result.clone());
            }
            return result;
        }
        if let Ok(api_key) = std::env::var("BRAVE_SEARCH_API_KEY") {
            let result = self.search_brave(&params, count, &api_key).await;
            if result.success {
                self.cache.set(cache_key, result.clone());
            }
            return result;
        }

        // Check for SerpAPI key (context first, then env)
        if let Some(api_key) = context.get_api_key("serpapi") {
            let result = self.search_serpapi(&params.query, count, &api_key).await;
            if result.success {
                self.cache.set(cache_key, result.clone());
            }
            return result;
        }
        if let Ok(api_key) = std::env::var("SERPAPI_API_KEY") {
            let result = self.search_serpapi(&params.query, count, &api_key).await;
            if result.success {
                self.cache.set(cache_key, result.clone());
            }
            return result;
        }

        ToolResult::error(
            "No search API configured. Add a Brave Search or SerpAPI key in the API Keys page, or set BRAVE_SEARCH_API_KEY or SERPAPI_API_KEY environment variable.",
        )
    }
}

impl WebSearchTool {
    async fn search_brave(&self, params: &WebSearchParams, count: u32, api_key: &str) -> ToolResult {
        let client = reqwest::Client::new();

        // Build URL with optional parameters
        let mut url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
            urlencoding::encode(&params.query),
            count
        );

        // Add optional country filter
        if let Some(ref country) = params.country {
            url.push_str(&format!("&country={}", country.to_lowercase()));
        }

        // Add optional freshness filter
        if let Some(ref freshness) = params.freshness {
            let freshness_value = match freshness.as_str() {
                "day" => "pd",      // past day
                "week" => "pw",     // past week
                "month" => "pm",    // past month
                "year" => "py",     // past year
                other => other,      // custom date range YYYY-MM-DD:YYYY-MM-DD
            };
            url.push_str(&format!("&freshness={}", freshness_value));
        }

        // Add optional search language
        if let Some(ref lang) = params.search_lang {
            url.push_str(&format!("&search_lang={}", lang.to_lowercase()));
        }

        let response = match client
            .get(&url)
            .header("X-Subscription-Token", api_key)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Failed to search: {}", e)),
        };

        if !response.status().is_success() {
            return ToolResult::error(format!(
                "Search API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let data: BraveSearchResponse = match response.json().await {
            Ok(d) => d,
            Err(e) => return ToolResult::error(format!("Failed to parse search results: {}", e)),
        };

        let results: Vec<SearchResult> = data
            .web
            .map(|w| {
                w.results
                    .into_iter()
                    .map(|r| SearchResult {
                        title: r.title,
                        url: r.url,
                        snippet: r.description,
                    })
                    .collect()
            })
            .unwrap_or_default();

        if results.is_empty() {
            return ToolResult::success("No results found for the query.");
        }

        let formatted = results
            .iter()
            .enumerate()
            .map(|(i, r)| format!("{}. {}\n   URL: {}\n   {}", i + 1, r.title, r.url, r.snippet))
            .collect::<Vec<_>>()
            .join("\n\n");

        ToolResult::success(formatted).with_metadata(json!({
            "results": results,
            "cached": false,
            "provider": "brave"
        }))
    }

    async fn search_serpapi(&self, query: &str, count: u32, api_key: &str) -> ToolResult {
        let client = reqwest::Client::new();
        let url = format!(
            "https://serpapi.com/search.json?q={}&api_key={}&num={}",
            urlencoding::encode(query),
            api_key,
            count
        );

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Failed to search: {}", e)),
        };

        if !response.status().is_success() {
            return ToolResult::error(format!(
                "Search API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let data: SerpApiResponse = match response.json().await {
            Ok(d) => d,
            Err(e) => return ToolResult::error(format!("Failed to parse search results: {}", e)),
        };

        let results: Vec<SearchResult> = data
            .organic_results
            .map(|r| {
                r.into_iter()
                    .map(|sr| SearchResult {
                        title: sr.title,
                        url: sr.link,
                        snippet: sr.snippet.unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        if results.is_empty() {
            return ToolResult::success("No results found for the query.");
        }

        let formatted = results
            .iter()
            .enumerate()
            .map(|(i, r)| format!("{}. {}\n   URL: {}\n   {}", i + 1, r.title, r.url, r.snippet))
            .collect::<Vec<_>>()
            .join("\n\n");

        ToolResult::success(formatted).with_metadata(json!({
            "results": results,
            "cached": false,
            "provider": "serpapi"
        }))
    }
}

// URL encoding helper
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut encoded = String::new();
        for c in s.chars() {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => encoded.push(c),
                ' ' => encoded.push_str("%20"),
                _ => {
                    for b in c.to_string().as_bytes() {
                        encoded.push_str(&format!("%{:02X}", b));
                    }
                }
            }
        }
        encoded
    }
}
