# LumaBet — Soroban Contract Reference

All contracts target `wasm32-unknown-unknown` and use `soroban-sdk v21`.

Build command:
```bash
cargo build --target wasm32-unknown-unknown --release
```

---

## `lumabet_rng`

**Purpose:** On-chain pseudo-random number generation.

### Functions

#### `generate_random(env, seed: u64, range: u64) -> u64`

Returns a number in **[1, range]** (inclusive).

| Param   | Type  | Description                                          |
|---------|-------|------------------------------------------------------|
| `seed`  | u64   | Caller-supplied entropy (e.g. hash of pubkey + ts)   |
| `range` | u64   | Inclusive upper bound. Must be ≥ 1.                  |

**Panics** if `range == 0`.

**Mixing algorithm:**
1. XOR `seed` with `ledger.sequence * 0x9e3779b97f4a7c15`
2. Add `ledger.timestamp * 0x6c62272e07bb0142`
3. Apply one LCG step (Knuth multiplier `6364136223846793005`)
4. Apply xorshift64 finaliser
5. Return `state % range + 1`

#### `generate_raw(env, seed: u64) -> u64`

Returns an unclamped 64-bit value. Equivalent to `generate_random(seed, u64::MAX - 1)`.

---

## `lumabet_core`

**Purpose:** Betting escrow — holds XLM, resolves outcomes, pays winners.

### Initialization

#### `initialize(env, admin: Address, xlm_token: Address, house_edge_bps: u32)`

Must be called once after deployment. Sets admin, token address, and house edge.

| Param            | Type    | Description                                   |
|------------------|---------|-----------------------------------------------|
| `admin`          | Address | Admin keypair address                         |
| `xlm_token`      | Address | Native XLM Stellar Asset Contract address     |
| `house_edge_bps` | u32     | House edge in basis points (200 = 2%)         |

---

### `place_bet(env, player: Address, amount: i128, game_type: Symbol, prediction: u64) -> u64`

Transfers `amount` stroops of XLM from `player` into contract escrow.

| Param        | Type    | Description                              |
|--------------|---------|------------------------------------------|
| `player`     | Address | Must authorize this call                 |
| `amount`     | i128    | Amount in stroops (must be > 0)          |
| `game_type`  | Symbol  | Game identifier, e.g. `DICE`            |
| `prediction` | u64     | Player's prediction                      |

**Returns** a `bet_id` (u64, equal to `ledger.timestamp` at placement time).

**Panics** if `amount <= 0`.

---

### `resolve_bet(env, bet_id: u64, outcome: u64, multiplier_bps: u32)`

Resolves a pending bet. Admin-only.

| Param            | Type | Description                                       |
|------------------|------|---------------------------------------------------|
| `bet_id`         | u64  | ID returned by `place_bet`                        |
| `outcome`        | u64  | Actual game outcome (e.g. 1–6 for dice)           |
| `multiplier_bps` | u32  | Gross payout multiplier in BPS (50000 = 5×)       |

**Win payout formula:**
```
gross_payout = amount * multiplier_bps / 10_000
house_cut    = gross_payout * house_edge_bps / 10_000
net_payout   = gross_payout - house_cut
```

**Panics** if bet not found or already resolved.

---

### `withdraw(env, recipient: Address, amount: i128)`

Admin-only. Withdraws house earnings from the contract.

---

### `get_balance(env) -> i128`

Returns the contract's current XLM balance in stroops.

---

### `get_bet(env, bet_id: u64) -> Bet`

Returns the `Bet` struct for a given ID. Panics if not found.

**`Bet` fields:**
| Field       | Type      | Description                        |
|-------------|-----------|------------------------------------|
| `player`    | Address   | Player's Stellar address           |
| `amount`    | i128      | Wagered amount in stroops          |
| `game_type` | Symbol    | Game identifier                    |
| `prediction`| u64       | Player's prediction                |
| `status`    | BetStatus | `Pending`, `Won`, `Lost`, `Withdrawn` |
| `payout`    | i128      | Net payout in stroops (0 if lost)  |
| `timestamp` | u64       | Ledger timestamp at placement      |

---

## `lumabet_dice`

**Purpose:** Dice game logic (1–6). Delegates randomness to `lumabet_rng`.

### `initialize(env, admin: Address, core_contract: Address, rng_contract: Address)`

One-time setup.

---

### `roll_dice(env, player: Address, bet_id: u64, prediction: u64, seed: u64) -> DiceRoll`

Generates a dice outcome and returns the result.

| Param        | Type    | Description                                  |
|--------------|---------|----------------------------------------------|
| `player`     | Address | Must authorize                               |
| `bet_id`     | u64     | Bet ID from `lumabet_core.place_bet`         |
| `prediction` | u64     | Player's guess (1–6)                         |
| `seed`       | u64     | Client entropy for the RNG                   |

**Returns `DiceRoll`:**
| Field        | Type    |
|--------------|---------|
| `player`     | Address |
| `prediction` | u64     |
| `outcome`    | u64     |
| `won`        | bool    |
| `bet_id`     | u64     |
| `timestamp`  | u64     |

**Panics** if `prediction` is not in 1–6.

---

### `payout_multiplier_bps(env) -> u32`

Returns `50000` (5× payout = 50,000 basis points).

---

### `claim_winnings(env, player: Address, bet_id: u64)`

Placeholder for player-initiated payout claim. In production, wire to
`lumabet_core.resolve_bet` via cross-contract call using the stored `DiceRoll`.
