# LumaBet Contract

Soroban smart contracts for the CelestialBet decentralized casino on Stellar XLM.

## Stack
- **Rust** + **Soroban SDK v21**
- Target: `wasm32-unknown-unknown`

## Contracts

| Crate           | Description                                     |
|-----------------|-------------------------------------------------|
| `lumabet_core`  | Betting escrow — holds XLM, resolves, pays out  |
| `lumabet_dice`  | Dice game logic (1–6), calls lumabet_rng        |
| `lumabet_rng`   | On-chain pseudo-RNG (LCG + xorshift64)          |

## Getting Started

```bash
rustup target add wasm32-unknown-unknown

# Build all contracts
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test

# Deploy to testnet (fill .env first)
cp .env.example .env
bash scripts/fund_testnet.sh YOUR_PUBLIC_KEY
bash scripts/deploy_contracts.sh
```

## Scripts

| Script                    | Description                              |
|---------------------------|------------------------------------------|
| `scripts/deploy_contracts.sh` | Build & deploy all three contracts   |
| `scripts/fund_testnet.sh`     | Fund accounts via Stellar Friendbot  |
| `scripts/seed_db.sql`         | PostgreSQL schema for bet history    |

## Docs

See [docs/contracts.md](docs/contracts.md) for full function reference.
