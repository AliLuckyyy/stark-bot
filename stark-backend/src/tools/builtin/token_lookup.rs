//! Token Lookup tool for resolving token symbols to addresses
//!
//! Provides a lookup table for known tokens on supported networks.
//! Token data is loaded from config/tokens.ron at startup.
//! This prevents hallucination of token addresses for common tokens.

use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Global token storage (loaded once at startup)
static TOKENS: OnceLock<HashMap<String, HashMap<String, TokenInfo>>> = OnceLock::new();

/// Token info loaded from config
#[derive(Debug, Clone, Deserialize)]
pub struct TokenInfo {
    pub address: String,
    pub decimals: u8,
    pub name: String,
}

/// Load tokens from config directory
pub fn load_tokens(config_dir: &Path) {
    let tokens_path = config_dir.join("tokens.ron");
    if tokens_path.exists() {
        match std::fs::read_to_string(&tokens_path) {
            Ok(content) => {
                match ron::from_str::<HashMap<String, HashMap<String, TokenInfo>>>(&content) {
                    Ok(tokens) => {
                        let total: usize = tokens.values().map(|t| t.len()).sum();
                        log::info!(
                            "[tokens] Loaded {} tokens across {} networks from {:?}",
                            total,
                            tokens.len(),
                            tokens_path
                        );
                        let _ = TOKENS.set(tokens);
                    }
                    Err(e) => {
                        log::error!("[tokens] Failed to parse tokens config: {}", e);
                        let _ = TOKENS.set(default_tokens());
                    }
                }
            }
            Err(e) => {
                log::error!("[tokens] Failed to read tokens file: {}", e);
                let _ = TOKENS.set(default_tokens());
            }
        }
    } else {
        log::warn!("[tokens] Tokens file not found: {:?}, using defaults", tokens_path);
        let _ = TOKENS.set(default_tokens());
    }
}

/// Get tokens, loading defaults if not already loaded
fn get_tokens() -> &'static HashMap<String, HashMap<String, TokenInfo>> {
    TOKENS.get_or_init(default_tokens)
}

/// Default tokens (fallback if config not found)
fn default_tokens() -> HashMap<String, HashMap<String, TokenInfo>> {
    let mut networks = HashMap::new();

    // Base network tokens
    let mut base = HashMap::new();
    base.insert("ETH".to_string(), TokenInfo {
        address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
        decimals: 18,
        name: "Ethereum".to_string(),
    });
    base.insert("WETH".to_string(), TokenInfo {
        address: "0x4200000000000000000000000000000000000006".to_string(),
        decimals: 18,
        name: "Wrapped Ether".to_string(),
    });
    base.insert("USDC".to_string(), TokenInfo {
        address: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
        decimals: 6,
        name: "USD Coin".to_string(),
    });
    networks.insert("base".to_string(), base);

    // Mainnet tokens
    let mut mainnet = HashMap::new();
    mainnet.insert("ETH".to_string(), TokenInfo {
        address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
        decimals: 18,
        name: "Ethereum".to_string(),
    });
    mainnet.insert("WETH".to_string(), TokenInfo {
        address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        decimals: 18,
        name: "Wrapped Ether".to_string(),
    });
    mainnet.insert("USDC".to_string(), TokenInfo {
        address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
        decimals: 6,
        name: "USD Coin".to_string(),
    });
    networks.insert("mainnet".to_string(), mainnet);

    networks
}

/// Token Lookup tool
pub struct TokenLookupTool {
    definition: ToolDefinition,
}

impl TokenLookupTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "symbol".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Token symbol (e.g., 'ETH', 'USDC', 'WETH'). Case-insensitive.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "network".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Network: 'base' or 'mainnet'".to_string(),
                default: Some(json!("base")),
                items: None,
                enum_values: Some(vec!["base".to_string(), "mainnet".to_string()]),
            },
        );

        properties.insert(
            "cache_as".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Register name to cache the token address (e.g., 'sell_token', 'buy_token')".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        TokenLookupTool {
            definition: ToolDefinition {
                name: "token_lookup".to_string(),
                description: "Look up a token's contract address by its symbol. Supports common tokens on Base and Mainnet. Use cache_as to store the address in a register for use with swap presets.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["symbol".to_string()],
                },
                group: ToolGroup::Web,
            },
        }
    }

    fn lookup(symbol: &str, network: &str) -> Option<TokenInfo> {
        let symbol_upper = symbol.to_uppercase();
        let tokens = get_tokens();

        tokens
            .get(network)
            .or_else(|| tokens.get("base"))
            .and_then(|network_tokens| network_tokens.get(&symbol_upper))
            .cloned()
    }

    fn list_available(network: &str) -> Vec<String> {
        let tokens = get_tokens();

        tokens
            .get(network)
            .or_else(|| tokens.get("base"))
            .map(|network_tokens| {
                let mut symbols: Vec<String> = network_tokens.keys().cloned().collect();
                symbols.sort();
                symbols
            })
            .unwrap_or_default()
    }
}

impl Default for TokenLookupTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct TokenLookupParams {
    symbol: String,
    #[serde(default = "default_network")]
    network: String,
    cache_as: Option<String>,
}

fn default_network() -> String {
    "base".to_string()
}

#[async_trait]
impl Tool for TokenLookupTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        let params: TokenLookupParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        match Self::lookup(&params.symbol, &params.network) {
            Some(token) => {
                // Cache in register if requested
                if let Some(ref register_name) = params.cache_as {
                    // Store address in the main register (e.g., "sell_token")
                    context.set_register(register_name, json!(&token.address), "token_lookup");

                    // Also store symbol in a separate register (e.g., "sell_token_symbol")
                    let symbol_register = format!("{}_symbol", register_name);
                    context.set_register(&symbol_register, json!(params.symbol.to_uppercase()), "token_lookup");

                    log::info!(
                        "[token_lookup] Cached {} in registers: '{}'={}, '{}'={}",
                        params.symbol,
                        register_name,
                        token.address,
                        symbol_register,
                        params.symbol.to_uppercase()
                    );
                }

                ToolResult::success(format!(
                    "{} ({}) on {}\nAddress: {}\nDecimals: {}",
                    token.name,
                    params.symbol.to_uppercase(),
                    params.network,
                    token.address,
                    token.decimals
                )).with_metadata(json!({
                    "symbol": params.symbol.to_uppercase(),
                    "address": token.address,
                    "decimals": token.decimals,
                    "name": token.name,
                    "network": params.network,
                    "cached_in_register": params.cache_as
                }))
            }
            None => {
                let available = Self::list_available(&params.network);
                ToolResult::error(format!(
                    "Token '{}' not found on {}. Available tokens: {}",
                    params.symbol,
                    params.network,
                    available.join(", ")
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_token_lookup() {
        let token = TokenLookupTool::lookup("USDC", "base").unwrap();
        assert_eq!(token.address, "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913");
        assert_eq!(token.decimals, 6);
    }

    #[test]
    fn test_case_insensitive() {
        let token1 = TokenLookupTool::lookup("usdc", "base").unwrap();
        let token2 = TokenLookupTool::lookup("USDC", "base").unwrap();
        let token3 = TokenLookupTool::lookup("Usdc", "base").unwrap();

        assert_eq!(token1.address, token2.address);
        assert_eq!(token2.address, token3.address);
    }

    #[test]
    fn test_eth_special_address() {
        let token = TokenLookupTool::lookup("ETH", "base").unwrap();
        assert_eq!(token.address, "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE");
    }

    #[test]
    fn test_unknown_token() {
        assert!(TokenLookupTool::lookup("UNKNOWN_TOKEN_XYZ", "base").is_none());
    }
}
