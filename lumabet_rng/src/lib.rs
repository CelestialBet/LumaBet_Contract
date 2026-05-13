//! LumaBet RNG — Pseudo-random number generation for Soroban
//!
//! Uses a combination of ledger sequence, timestamp, and caller-supplied seed
//! to produce deterministic-but-unpredictable randomness within a single ledger.
//! For production, replace with a VRF oracle or Stellar's native randomness once available.

#![no_std]

use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct LumaBetRng;

#[contractimpl]
impl LumaBetRng {
    /// Generate a pseudo-random number in [1, range] (inclusive).
    ///
    /// `seed`  — caller-supplied entropy (e.g. derived from player public key hash)
    /// `range` — upper bound, result is in [1, range]
    ///
    /// Mixing strategy: XOR of ledger sequence, timestamp, and seed,
    /// then folded through a 64-bit LCG step (Knuth multiplicative).
    pub fn generate_random(env: Env, seed: u64, range: u64) -> u64 {
        if range == 0 {
            panic!("range must be at least 1");
        }

        let seq = env.ledger().sequence() as u64;
        let ts = env.ledger().timestamp();

        // Mix sources
        let mut state: u64 = seed ^ (seq.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        state = state.wrapping_add(ts.wrapping_mul(0x6c62_272e_07bb_0142));

        // LCG step: multiplier and addend from Knuth
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);

        // Additional mixing — xorshift64
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state = state.wrapping_mul(0x2545_f491_4f6c_dd1d);

        (state % range) + 1
    }

    /// Generate a raw 64-bit random value (no range clamping).
    pub fn generate_raw(env: Env, seed: u64) -> u64 {
        LumaBetRng::generate_random(env, seed, u64::MAX - 1)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_dice_range() {
        let env = Env::default();
        let contract_id = env.register(LumaBetRng, ());
        let client = LumaBetRngClient::new(&env, &contract_id);

        for seed in 0u64..50 {
            let result = client.generate_random(&seed, &6u64);
            assert!(result >= 1 && result <= 6, "result out of dice range: {}", result);
        }
    }

    #[test]
    fn test_range_1_returns_1() {
        let env = Env::default();
        let contract_id = env.register(LumaBetRng, ());
        let client = LumaBetRngClient::new(&env, &contract_id);
        assert_eq!(client.generate_random(&42u64, &1u64), 1);
    }

    #[test]
    #[should_panic(expected = "range must be at least 1")]
    fn test_zero_range_panics() {
        let env = Env::default();
        let contract_id = env.register(LumaBetRng, ());
        let client = LumaBetRngClient::new(&env, &contract_id);
        client.generate_random(&0u64, &0u64);
    }

    #[test]
    fn test_different_seeds_produce_variance() {
        let env = Env::default();
        let contract_id = env.register(LumaBetRng, ());
        let client = LumaBetRngClient::new(&env, &contract_id);

        let results: soroban_sdk::Vec<u64> = {
            let mut v = soroban_sdk::Vec::new(&env);
            for seed in 0u64..20 {
                v.push_back(client.generate_random(&seed, &100u64));
            }
            v
        };

        // At least some values should differ (i.e., distribution exists)
        let first = results.get(0).unwrap();
        let has_variance = results.iter().any(|v| v != first);
        assert!(has_variance, "all values identical — broken RNG");
    }
}
