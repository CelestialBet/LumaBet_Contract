//! LumaBet Dice — On-chain dice game (1–6)
//!
//! Players predict which number (1–6) a dice roll lands on.
//! Randomness is sourced from lumabet_rng via cross-contract call.
//! Payouts are 5x minus house edge (probability of any face = 1/6).

#![no_std]

use soroban_sdk::{
    contract, contractclient, contractimpl, contracttype, log, symbol_short, Address, Env, Symbol,
};

const ADMIN: Symbol = symbol_short!("ADMIN");
const CORE_CONTRACT: Symbol = symbol_short!("CORE");
const RNG_CONTRACT: Symbol = symbol_short!("RNG");
const PAYOUT_BPS: u32 = 50_000; // 5x payout = 50,000 basis points

// ── RNG cross-contract interface ──────────────────────────────────────────────
// Declared as a trait so no pre-built WASM file is needed at compile time.

#[contractclient(name = "RngClient")]
pub trait RngInterface {
    fn generate_random(env: Env, seed: u64, range: u64) -> u64;
}

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct DiceRoll {
    pub player: Address,
    pub prediction: u64,
    pub outcome: u64,
    pub won: bool,
    pub bet_id: u64,
    pub timestamp: u64,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct LumaBetDice;

#[contractimpl]
impl LumaBetDice {
    /// Initialize with references to the core escrow and RNG contracts.
    pub fn initialize(env: Env, admin: Address, core_contract: Address, rng_contract: Address) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialized");
        }
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&CORE_CONTRACT, &core_contract);
        env.storage().instance().set(&RNG_CONTRACT, &rng_contract);
    }

    /// Roll the dice. The player must have already called `lumabet_core::place_bet`.
    ///
    /// `bet_id`     — ID returned from place_bet
    /// `prediction` — player's guess (1–6)
    /// `seed`       — entropy supplied by the client (e.g. hash of pubkey + timestamp)
    pub fn roll_dice(
        env: Env,
        player: Address,
        bet_id: u64,
        prediction: u64,
        seed: u64,
    ) -> DiceRoll {
        player.require_auth();

        if prediction < 1 || prediction > 6 {
            panic!("prediction must be between 1 and 6");
        }

        let rng_address: Address = env.storage().instance().get(&RNG_CONTRACT).unwrap();
        let rng_client = RngClient::new(&env, &rng_address);

        // Generate dice outcome (1–6) via cross-contract call
        let outcome = rng_client.generate_random(&seed, &6u64);
        let won = prediction == outcome;

        log!(
            &env,
            "roll_dice: player={}, prediction={}, outcome={}, won={}",
            player,
            prediction,
            outcome,
            won
        );

        DiceRoll {
            player,
            prediction,
            outcome,
            won,
            bet_id,
            timestamp: env.ledger().timestamp(),
        }
    }

    /// Returns the gross payout multiplier in basis points (5x = 50,000 bps).
    pub fn payout_multiplier_bps(_env: Env) -> u32 {
        PAYOUT_BPS
    }

    /// Claim winnings for a resolved dice roll. Only callable by the player.
    pub fn claim_winnings(env: Env, player: Address, bet_id: u64) {
        player.require_auth();
        log!(&env, "claim_winnings: player={}, bet_id={}", player, bet_id);
        // Wire to lumabet_core::resolve_bet via cross-contract call in production.
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn test_payout_multiplier() {
        let env = Env::default();
        let contract_id = env.register(LumaBetDice, ());
        let client = LumaBetDiceClient::new(&env, &contract_id);
        assert_eq!(client.payout_multiplier_bps(), 50_000u32);
    }

    #[test]
    fn test_initialize_stores_contracts() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let core = Address::generate(&env);
        let rng = Address::generate(&env);

        let contract_id = env.register(LumaBetDice, ());
        let client = LumaBetDiceClient::new(&env, &contract_id);
        // Should not panic
        client.initialize(&admin, &core, &rng);
    }

    #[test]
    #[should_panic(expected = "already initialized")]
    fn test_double_initialize_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let core = Address::generate(&env);
        let rng = Address::generate(&env);

        let contract_id = env.register(LumaBetDice, ());
        let client = LumaBetDiceClient::new(&env, &contract_id);
        client.initialize(&admin, &core, &rng);
        client.initialize(&admin, &core, &rng); // should panic
    }
}
