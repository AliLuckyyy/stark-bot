---
name: swap
description: "Swap ERC20 tokens on Base using 0x DEX aggregator via quoter.defirelay.com"
version: 4.3.0
author: starkbot
homepage: https://0x.org
metadata: {"requires_auth": false, "clawdbot":{"emoji":"🔄"}}
tags: [crypto, defi, swap, dex, base, trading, 0x]
---

# Token Swap Integration (0x via DeFi Relay)

## EXACT TOOL CALLS - Copy These Exactly!

To swap tokens, make these EXACT tool calls in order:

### 1. local_burner_wallet
```json
{"action": "address", "cache_as": "wallet_address"}
```

### 2. token_lookup (sell token) - MUST include cache_as!
```json
{"symbol": "ETH", "network": "base", "cache_as": "sell_token"}
```

### 3. token_lookup (buy token) - MUST include cache_as!
```json
{"symbol": "USDC", "network": "base", "cache_as": "buy_token"}
```

### 4. register_set (amount in wei)
```json
{"key": "sell_amount", "value": "100000000000000"}
```

### 5. x402_fetch
```json
{"preset": "swap_quote", "network": "base", "cache_as": "swap_quote"}
```

### 6. x402_rpc
```json
{"preset": "gas_price", "network": "base"}
```

### 7. web3_tx
```json
{"from_register": "swap_quote", "max_fee_per_gas": "<GAS_PRICE_FROM_STEP_6>", "network": "base"}
```

---

## CRITICAL RULES

### token_lookup MUST include cache_as in the SAME call!

**WRONG - Two separate calls:**
```json
// Step 1 - lookup
{"symbol": "ETH", "network": "base"}
// Step 2 - try to store (WILL FAIL!)
{"key": "sell_token", "value": "0xEeee..."}  // BLOCKED!
```

**CORRECT - One call with cache_as:**
```json
{"symbol": "ETH", "network": "base", "cache_as": "sell_token"}
```

### You CANNOT use register_set for:
- `sell_token` - use `token_lookup` with `cache_as: "sell_token"`
- `buy_token` - use `token_lookup` with `cache_as: "buy_token"`
- `wallet_address` - use `local_burner_wallet` with `cache_as: "wallet_address"`

These registers are BLOCKED from register_set to prevent errors.

---

## Amount Reference (Wei Values)

For ETH (18 decimals):
- 0.0001 ETH = `100000000000000`
- 0.001 ETH = `1000000000000000`
- 0.01 ETH = `10000000000000000`
- 0.1 ETH = `100000000000000000`
- 1 ETH = `1000000000000000000`

For USDC (6 decimals):
- 1 USDC = `1000000`
- 10 USDC = `10000000`
- 100 USDC = `100000000`

---

## Supported Tokens

ETH, WETH, USDC, USDbC, DAI, cbBTC, BNKR, AERO, DEGEN, BRETT, TOSHI

---

## What Gets Stored in Registers

After running the steps above:

| Register | Source | Example Value |
|----------|--------|---------------|
| `wallet_address` | local_burner_wallet | `0x57bf3c9d...` |
| `sell_token` | token_lookup | `0xEeee...` |
| `sell_token_symbol` | token_lookup | `ETH` |
| `buy_token` | token_lookup | `0x8335...` |
| `buy_token_symbol` | token_lookup | `USDC` |
| `sell_amount` | register_set | `100000000000000` |
| `network_name` | x402_fetch | `Base` |
| `chain_id` | x402_fetch | `8453` |
| `swap_quote` | x402_fetch | `{to, data, value, gas, ...}` |

---

## Error Handling

| Error | Fix |
|-------|-----|
| "Cannot set 'sell_token' via register_set" | Use `token_lookup` with `cache_as: "sell_token"` instead! |
| "Cannot set 'buy_token' via register_set" | Use `token_lookup` with `cache_as: "buy_token"` instead! |
| "Preset requires register 'X'" | Run the tool that sets register X first |
| "Insufficient balance" | Check balance before swapping |
