# Register System v2 - Full Anti-Hallucination

## Problem Statement

Even with registers for tx data, the agent can still hallucinate:
1. **Wallet address** - wrong taker address in quote URL
2. **Token addresses** - swap wrong tokens
3. **Amounts** - swap wrong amount
4. **URL construction** - malformed or manipulated URLs

## Solution: Complete Register-Based Flow

Every piece of critical data flows through registers. The agent only:
1. Invokes tools with preset names
2. Passes register keys (not values)
3. Sets validated user inputs into registers

```
USER INPUT: "swap 0.001 ETH for USDC"
                ↓
        [Agent parses intent]
                ↓
┌─────────────────────────────────────────────────────────────┐
│                    REGISTER STORE                            │
├─────────────────────────────────────────────────────────────┤
│ wallet_address  ← local_burner_wallet (auto-cached)         │
│ sell_token      ← register_set (from skill's token table)   │
│ buy_token       ← register_set (from skill's token table)   │
│ sell_amount     ← register_set (validated amount in wei)    │
│ swap_quote      ← x402_fetch preset:swap_quote (reads above)│
└─────────────────────────────────────────────────────────────┘
                ↓
        [web3_tx from_register: "swap_quote"]
```

## Implementation Plan

### Phase 1: Extend local_burner_wallet

Add `cache_as` parameter to store the wallet address in a register.

```rust
// local_burner_wallet params
{
  "action": "address",
  "cache_as": "wallet_address"  // NEW: stores result in register
}
```

**Files to modify:**
- `src/tools/builtin/local_burner_wallet.rs`

### Phase 2: Create register_set Tool

A new tool that lets skills set validated values into registers.

```rust
// register_set tool
{
  "key": "sell_token",
  "value": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
}
```

The tool validates:
- Key names (alphanumeric + underscore only)
- Values are valid for their type (addresses are checksummed, amounts are numeric)

**Files to create:**
- `src/tools/builtin/register_set.rs`

### Phase 3: Preset Destinations for x402_fetch

Add preset URL builders that read from registers instead of accepting raw URLs.

```rust
// x402_fetch with preset
{
  "preset": "swap_quote",
  "network": "base",
  "cache_as": "swap_quote"
}

// The preset internally builds:
// https://quoter.defirelay.com/swap/allowance-holder/quote
//   ?chainId={chain_id_for_network}
//   &sellToken={register:sell_token}
//   &buyToken={register:buy_token}
//   &sellAmount={register:sell_amount}
//   &taker={register:wallet_address}
```

**Presets to implement:**

| Preset | Registers Read | Purpose |
|--------|----------------|---------|
| `swap_quote` | wallet_address, sell_token, buy_token, sell_amount | Get 0x swap quote |
| `token_price` | token_address | Get token price |

**Files to modify:**
- `src/tools/builtin/x402_fetch.rs`

### Phase 4: Update Swap Skill

New flow in `skills/swap.md`:

```markdown
### Step 1: Get Wallet Address (cached automatically)

```json
// local_burner_wallet - address is cached in "wallet_address" register
{"action": "address", "cache_as": "wallet_address"}
```

### Step 2: Set Swap Parameters

```json
// register_set - set the tokens and amount
{"key": "sell_token", "value": "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"}
{"key": "buy_token", "value": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"}
{"key": "sell_amount", "value": "10000000000000000"}
```

### Step 3: Get Quote (uses preset, reads from registers)

```json
// x402_fetch - preset builds URL from registers
{
  "preset": "swap_quote",
  "network": "base",
  "cache_as": "swap_quote"
}
```

### Step 4: Execute Swap

```json
// web3_tx - reads tx data from register
{
  "from_register": "swap_quote",
  "max_fee_per_gas": "...",
  "network": "base"
}
```
```

## Data Flow Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                         SKILL FILE                                │
│  (defines token addresses, preset names, register keys)          │
└──────────────────────────────────────────────────────────────────┘
                                ↓
                        [Agent reads skill]
                                ↓
┌──────────────────────────────────────────────────────────────────┐
│                      TOOL CALLS (agent)                          │
│                                                                   │
│  1. local_burner_wallet {action: "address", cache_as: "wallet"}  │
│  2. register_set {key: "sell_token", value: "<from skill>"}      │
│  3. register_set {key: "buy_token", value: "<from skill>"}       │
│  4. register_set {key: "sell_amount", value: "<calculated>"}     │
│  5. x402_fetch {preset: "swap_quote", cache_as: "swap_quote"}    │
│  6. web3_tx {from_register: "swap_quote", max_fee_per_gas: "..."}│
└──────────────────────────────────────────────────────────────────┘
                                ↓
┌──────────────────────────────────────────────────────────────────┐
│                     REGISTER STORE (backend)                     │
│                                                                   │
│  wallet_address: "0x1234..."      ← from local_burner_wallet     │
│  sell_token: "0xEeee..."          ← from register_set            │
│  buy_token: "0x8335..."           ← from register_set            │
│  sell_amount: "10000000000000000" ← from register_set            │
│  swap_quote: {to, data, value, gas, ...} ← from x402_fetch       │
└──────────────────────────────────────────────────────────────────┘
                                ↓
┌──────────────────────────────────────────────────────────────────┐
│                   PRESET URL BUILDER (x402_fetch)                │
│                                                                   │
│  Input: preset="swap_quote", network="base"                      │
│  Reads: registers[wallet_address, sell_token, buy_token, ...]    │
│  Builds: https://quoter.defirelay.com/swap/allowance-holder/quote│
│          ?chainId=8453                                            │
│          &sellToken=0xEeee...                                     │
│          &buyToken=0x8335...                                      │
│          &sellAmount=10000000000000000                            │
│          &taker=0x1234...                                         │
│  Output: → register["swap_quote"]                                 │
└──────────────────────────────────────────────────────────────────┘
```

## Security Properties

1. **Wallet address**: Auto-cached from actual wallet, never typed by agent
2. **Token addresses**: Set via register_set from skill-defined constants
3. **Amounts**: Set via register_set, can be validated
4. **URL construction**: Done by preset builder, not agent
5. **TX data**: Read from register, not passed by agent

## Implementation Order

1. **Phase 1**: Add `cache_as` to `local_burner_wallet` (simple)
2. **Phase 2**: Create `register_set` tool (new tool)
3. **Phase 3**: Add preset system to `x402_fetch` (most complex)
4. **Phase 4**: Update swap skill to use new flow
5. **Phase 5**: Add validation (checksummed addresses, numeric amounts)

## Files to Modify/Create

| File | Action | Description |
|------|--------|-------------|
| `src/tools/builtin/local_burner_wallet.rs` | Modify | Add cache_as parameter |
| `src/tools/builtin/register_set.rs` | Create | New tool for setting register values |
| `src/tools/builtin/x402_fetch.rs` | Modify | Add preset URL builder system |
| `src/tools/builtin/mod.rs` | Modify | Export new tool |
| `src/tools/mod.rs` | Modify | Register new tool |
| `skills/swap.md` | Modify | Update to v4.0.0 with full register flow |

## Open Questions

1. Should `register_set` validate that addresses are checksummed?
2. Should we have a `register_get` tool for debugging?
3. Should presets be configurable via config file or hardcoded?
4. How to handle token decimals? (Agent still needs to calculate wei amounts)
