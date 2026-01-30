---
name: transfer
description: "Transfer ETH or ERC20 tokens on Base/Ethereum using the burner wallet"
version: 3.0.0
author: starkbot
homepage: https://basescan.org
metadata: {"requires_auth": false, "clawdbot":{"emoji":"💸","requires":{"bins":[]}}}
requires_binaries: []
tags: [crypto, transfer, send, eth, erc20, base, wallet]
---

# Token Transfer Skill

Transfer ETH or ERC20 tokens from the burner wallet to any address.

> **IMPORTANT: This skill uses the REGISTER PATTERN to prevent hallucination of transaction data.**
>
> - Transaction params are stored in registers using `register_set`
> - `web3_tx` reads from the register using `from_register`
> - You NEVER pass raw tx params directly to web3_tx

## Tools Used

| Tool | Purpose |
|------|---------|
| `local_burner_wallet` | Get wallet address and check balances |
| `register_set` | Build transaction params in a register |
| `x402_rpc` | Get gas price |
| `web3_tx` | Execute transfer from register |
| `web3_function_call` | Transfer ERC20 tokens |

---

## How to Transfer ETH

### Step 1: Get Gas Price

```json
// x402_rpc
{"preset": "gas_price", "network": "base"}
```

### Step 2: Build Transfer in Register

Use `register_set` with `json_value` to store the tx data:

```json
// register_set
{
  "key": "transfer_tx",
  "json_value": {
    "to": "<RECIPIENT_ADDRESS>",
    "value": "<AMOUNT_IN_WEI>",
    "data": "0x",
    "gas": "21000"
  }
}
```

### Step 3: Execute Transfer

```json
// web3_tx
{
  "from_register": "transfer_tx",
  "max_fee_per_gas": "<GAS_PRICE>",
  "network": "base"
}
```

---

## Complete Example: Send 0.01 ETH

### 1. Get gas price

```json
// x402_rpc
{"preset": "gas_price", "network": "base"}
```
Response: `"0xf4240"`

### 2. Build tx in register

```json
// register_set
{
  "key": "transfer_tx",
  "json_value": {
    "to": "0x1234567890abcdef1234567890abcdef12345678",
    "value": "10000000000000000",
    "data": "0x",
    "gas": "21000"
  }
}
```

### 3. Execute

```json
// web3_tx
{
  "from_register": "transfer_tx",
  "max_fee_per_gas": "0xf4240",
  "network": "base"
}
```

---

## Transfer ERC20 Tokens

For ERC20 transfers, use `web3_function_call` directly (it handles encoding):

```json
// web3_function_call
{
  "abi": "erc20",
  "contract": "<TOKEN_ADDRESS>",
  "function": "transfer",
  "params": [
    "<RECIPIENT_ADDRESS>",
    "<AMOUNT_IN_SMALLEST_UNIT>"
  ],
  "network": "base"
}
```

### Example: Send 10 USDC

USDC has 6 decimals, so 10 USDC = `10000000`

```json
// web3_function_call
{
  "abi": "erc20",
  "contract": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
  "function": "transfer",
  "params": [
    "0x1234567890abcdef1234567890abcdef12345678",
    "10000000"
  ],
  "network": "base"
}
```

---

## Check Balances

### Check ETH Balance

```json
// local_burner_wallet
{"action": "balance", "network": "base"}
```

### Check ERC20 Token Balance

```json
// web3_function_call
{
  "abi": "erc20",
  "contract": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
  "function": "balanceOf",
  "params": ["<WALLET_ADDRESS>"],
  "network": "base",
  "call_only": true
}
```

---

## Common Token Addresses (Base)

Use `token_lookup` to get addresses automatically, or use these directly:

| Token | Address | Decimals |
|-------|---------|----------|
| USDC | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` | 6 |
| WETH | `0x4200000000000000000000000000000000000006` | 18 |
| BNKR | `0x22aF33FE49fD1Fa80c7149773dDe5890D3c76F3b` | 18 |
| cbBTC | `0xcbB7C0000aB88B473b1f5aFd9ef808440eed33Bf` | 8 |
| DAI | `0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb` | 18 |
| USDbC | `0xd9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA` | 6 |

---

## Amount Conversion Reference

| Token | Decimals | Human Amount | Wei Value |
|-------|----------|--------------|-----------|
| ETH | 18 | 0.01 | `10000000000000000` |
| ETH | 18 | 0.1 | `100000000000000000` |
| ETH | 18 | 1 | `1000000000000000000` |
| USDC | 6 | 1 | `1000000` |
| USDC | 6 | 10 | `10000000` |
| USDC | 6 | 100 | `100000000` |
| BNKR | 18 | 1 | `1000000000000000000` |
| BNKR | 18 | 100 | `100000000000000000000` |
| cbBTC | 8 | 0.001 | `100000` |
| cbBTC | 8 | 0.01 | `1000000` |

---

## Pre-Transfer Checklist

Before executing a transfer:

1. **Verify recipient address** - Double-check the address is correct
2. **Check balance** - Use `local_burner_wallet` or `web3_function_call` (balanceOf)
3. **Confirm amount** - Ensure decimals are correct for the token
4. **Network** - Confirm you're on the right network (base vs mainnet)

---

## Error Handling

| Error | Cause | Solution |
|-------|-------|----------|
| "Insufficient funds" | Not enough ETH for gas | Add ETH to wallet |
| "Transfer amount exceeds balance" | Not enough tokens | Check token balance |
| "Gas estimation failed" | Invalid recipient or params | Verify addresses |
| "Transaction reverted" | Contract rejection | Check amounts |
| "Register not found" | Missing register | Use register_set first |

---

## Security Notes

1. **Register pattern prevents hallucination** - tx data flows through registers
2. **Always double-check addresses** - Transactions cannot be reversed
3. **Start with small test amounts** - Verify the flow works first
4. **Verify token contracts** - Use official addresses from block explorer
5. **Gas costs** - ETH needed for gas even when sending ERC20s
