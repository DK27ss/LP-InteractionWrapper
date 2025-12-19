# PancakeV3Pool UniversalWrapper

## Overview

- **universal-cli**: Universal Wrapper for any PancakeV3Pool

---

### Configuration
```bash
# Via argument
./universal-cli -c <CONTRACT> <command>

# Via environment variable
export CONTRACT_ADDRESS=0x...
./universal-cli <command>
```

### Commands

#### Configure target pool

```bash
# Auto-detects token0, token1, fee
./universal-cli -c <CONTRACT> set-pool --pool 0xPoolV3Address
```

#### View configuration

```bash
./universal-cli -c <CONTRACT> info
```
Output:
```
=== Pool Configuration ===
Pool:       0x...
Base:       0x...USDT
Quote:      0x...TOKEN
Fee:        10000 (1%)
Tick:       -7536
Ticks def:  [-9200, -7000]

=== Balances ===
Base:  50.0
Quote: 1000.0

=== Positions: 3 active ===
```

#### Configure default ticks

```bash
./universal-cli -c <CONTRACT> set-ticks --lower -9200 --upper -7000
```

#### ADD - Create positions

```bash
# Single position
./universal-cli -c <CONTRACT> add --base 1.0 --quote 100

# Batch - 5 positions
./universal-cli -c <CONTRACT> add --base 0.5 --quote 50 --count 5
```

#### REMOVE - Close positions

```bash
# By ID
./universal-cli -c <CONTRACT> remove --id 1234567

# Batch
./universal-cli -c <CONTRACT> remove --count 5

# All
./universal-cli -c <CONTRACT> remove --all
```

#### REBALANCE

```bash
# Single
./universal-cli -c <CONTRACT> rebalance --id 1234567

# Batch (N×REMOVE then N×ADD)
./universal-cli -c <CONTRACT> rebalance --count 3
```

#### BUY - Swap base → quote

```bash
./universal-cli -c <CONTRACT> buy --amount 10
./universal-cli -c <CONTRACT> buy --amount 2 --count 5
```

#### SELL - Swap quote → base

```bash
./universal-cli -c <CONTRACT> sell --amount 100
./universal-cli -c <CONTRACT> sell --amount 20 --count 5
```

#### FUND - Send tokens

```bash
# Send USDT
./universal-cli -c <CONTRACT> fund --token 0x55d398326f99059fF775485246999027B3197955 --amount 50

# Send another token
./universal-cli -c <CONTRACT> fund --token 0xTokenAddress --amount 1000
```

#### WITHDRAW

```bash
# Withdraw base + quote
./universal-cli -c <CONTRACT> withdraw

# Withdraw specific token
./universal-cli -c <CONTRACT> withdraw-token --token 0xTokenAddress
```

#### Balance & Positions

```bash
./universal-cli -c <CONTRACT> balance
./universal-cli -c <CONTRACT> positions
```

---

## Contract Deployment

### Universal Contract (UniversalTrigger)

```bash
forge create contracts/UniversalTrigger.sol:UniversalTrigger --rpc-url <RPC> --private-key $PRIVATE_KEY --broadcast
```

---

## Workflow - New Token

```bash
# 1. Deploy UniversalTrigger
forge create contracts/UniversalTrigger.sol:UniversalTrigger --rpc-url <RPC> --private-key $PK --broadcast
# Output: Deployed to: 0xNewContract

# 2. Configure pool
./universal-cli -c <DEPLOYED_CONTRACT> set-pool --pool 0xPoolV3

# 3. Verify config
./universal-cli -c <DEPLOYED_CONTRACT> info

# 4. Send funds
./universal-cli -c <DEPLOYED_CONTRACT> fund --token 0xUSDT --amount 100
./universal-cli -c <DEPLOYED_CONTRACT> fund --token 0xTOKEN --amount 5000

# 5. Trigger bots with ADD
./universal-cli -c <DEPLOYED_CONTRACT> add --base 2 --quote 100 --count 10

# 6. Or use REBALANCE (REMOVE→ADD in 1 TX)
./universal-cli -c <DEPLOYED_CONTRACT> rebalance --count 5

# 7. Recover funds
./universal-cli -c <DEPLOYED_CONTRACT> remove --all
./universal-cli -c <DEPLOYED_CONTRACT> withdraw
```

### Optimal Pattern

```
┌─────────────────────────────────────────────────────────────┐
│  REBALANCE BATCH: N×REMOVE then N×ADD                       │
│                                                             │
│  Events emitted:                                            │
│  1. DecreaseLiquidity (REMOVE)                              │
│  2. DecreaseLiquidity (REMOVE)                              │
│  3. DecreaseLiquidity (REMOVE)                              │
│  ...                                                        │
│  N+1. Mint (ADD)  ←                                         │
│  N+2. Mint (ADD)  ← LAST EVENTS                             │
│  N+3. Mint (ADD)  ←                                         │
│                                                             │
│  Bots see ADD as last event → BUY CASCADE                   │
└─────────────────────────────────────────────────────────────┘
```

---

## Useful Addresses (BSC)

| Name | Address |
|------|---------|
| USDT | `0x55d398326f99059fF775485246999027B3197955` |
| BEAT | `0xcf3232B85b43BCa90E51D38cc06Cc8bB8C8A3E36` |
| BEAT Pool V3 | `0xE5f1395eFce39A2AC238B63f79DbC5d524C85dcc` |
| Position Manager | `0x7b8A01B39D58278b5DE7e48c8449c9f4F5170613` |
| Beat-CLI Contract | `0x31a3c5ee6935086442f097AB3e5f371e1b06780a` |

---

### Event-Based Bot Behavior

Bots monitor PancakeSwap V3 events in real-time:

| Event | Bot Interpretation | Action |
|-------|-------------------|--------|
| `IncreaseLiquidity` (ADD) | "Someone believes in the project" | BUY |
| `DecreaseLiquidity` (REMOVE) | "LP is exiting, danger" | SELL |

### Key Insight

```
Scenario 1: REMOVE then ADD (last = ADD)
  [REMOVE] → [ADD] → Bots see "ADD" → BUY

Scenario 2: ADD then REMOVE (last = REMOVE)
  [ADD] → [REMOVE] → Bots see "REMOVE" → SELL
```

### Why REBALANCE Works

The `rebalance` function executes in a single transaction:
1. First: All REMOVE operations (DecreaseLiquidity events)
2. Last: All ADD operations (Mint events)

Since ADDs are the **last events**, bots interpret this as a buy signal.
