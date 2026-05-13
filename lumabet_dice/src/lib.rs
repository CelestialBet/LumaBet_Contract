//! LumaBet Dice — On-chain dice game (1–6)
//!
//! Players predict which number (1–6) a dice roll lands on.
//! Randomness is sourced from lumabet_rng. Payouts are 5x minus house edge,
//! since the probability of hitting any single face is 1/6.

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, log, symbol_short, Address, BytesN, Env, Symbol,
};

const ADMIN: Symbol = symbol_short!("ADMIN");
const CORE_CONTRACT: Symbol = symbol_short!("CORE");
const RNG_CONTRACT: Symbol = symbol_short!("RNG");
const PAYOUT_BPS: u32 = 50_000; // 5x payout = 50,000 basis points

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

// RNG cross-contract interface
mod rng_interface {
    soroban_sdk::contractimport!(
        file = "../lumabet_rng/target/wasm32-unknown-unknown/release/lumabet_rng.wasm"
    );
}

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
    /// This function resolves the RNG, determines outcome, and calls back to core.
    ///
    /// `bet_id`     — ID returned from place_bet
    /// `prediction` — player's guess (1–6)
    /// `seed`       — entropy from the client (e.g. hash of player pubkey + timestamp)
    pub fn roll_dice(env: Env, player: Address, bet_id: u64, prediction: u64, seed: u64) -> DiceRoll {
        player.require_auth();

        if prediction < 1 || prediction > 6 {
            panic!("prediction must be between 1 and 6");
        }

        let rng_contract: Address = env.storage().instance().get(&RNG_CONTRACT).unwrap();
        let rng_client = rng_interface::Client::new(&env, &rng_contract);

        // Generate the dice outcome (1–6)
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

        let roll = DiceRoll {
            player: player.clone(),
            prediction,
            outcome,
            won,
            bet_id,
            timestamp: env.ledger().timestamp(),
        };

        roll
    }

    /// Convenience: compute the payout basis points for a dice win.
    /// External callers (e.g. the API) can use this to display expected payout.
    pub fn payout_multiplier_bps(_env: Env) -> u32 {
        PAYOUT_BPS
    }

    /// Claim winnings for a resolved dice roll. Only callable by the player.
    pub fn claim_winnings(env: Env, player: Address, bet_id: u64) {
        player.require_auth();
        log!(&env, "claim_winnings: player={}, bet_id={}", player, bet_id);
        // In a full implementation, this validates the DiceRoll result stored
        // in instance storage and invokes core::resolve_bet on behalf of the player.
        // Storing roll results omitted here for brevity — wire via events in production.
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    // Note: Full integration tests require deploying the RNG contract.
    // Unit tests below cover contract initialization and validation logic.

    #[test]
    fn test_invalid_prediction_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        let core = Address::generate(&env);
        let rng = Address::generate(&env);

        let contract_id = env.register(LumaBetDice, ());
        let client = LumaBetDiceClient::new(&env, &contract_id);
        client.initialize(&admin, &core, &rng);

        // prediction = 0 should panic
        let result = std::panic::catch_unwind(|| {
            client.roll_dice(&player, &1u64, &0u64, &12345u64);
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_payout_multiplier() {
        let env = Env::default();
        let contract_id = env.register(LumaBetDice, ());
        let client = LumaBetDiceClient::new(&env, &contract_id);
        // 5x payout = 50,000 bps
        assert_eq!(client.payout_multiplier_bps(), 50_000u32);
    }
}
