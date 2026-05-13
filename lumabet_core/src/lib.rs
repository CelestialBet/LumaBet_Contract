//! LumaBet Core — Betting escrow contract
//!
//! Manages XLM bets: placement, resolution, and withdrawals.
//! All funds are held in escrow by the contract until a game resolves.

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, log, symbol_short, token, Address, Env, Map, Symbol,
};

// ── Storage Keys ──────────────────────────────────────────────────────────────

const BET_MAP: Symbol = symbol_short!("BET_MAP");
const ADMIN: Symbol = symbol_short!("ADMIN");
const XLM_TOKEN: Symbol = symbol_short!("XLM_TOK");
const HOUSE_EDGE_BPS: Symbol = symbol_short!("HOUSE_BP"); // basis points (e.g. 200 = 2%)

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum BetStatus {
    Pending,
    Won,
    Lost,
    Withdrawn,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Bet {
    pub player: Address,
    pub amount: i128,
    pub game_type: Symbol,
    pub prediction: u64,
    pub status: BetStatus,
    pub payout: i128,
    pub timestamp: u64,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct LumaBetCore;

#[contractimpl]
impl LumaBetCore {
    /// Initialize the contract with admin, XLM token address, and house edge.
    pub fn initialize(env: Env, admin: Address, xlm_token: Address, house_edge_bps: u32) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialized");
        }
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&XLM_TOKEN, &xlm_token);
        env.storage().instance().set(&HOUSE_EDGE_BPS, &house_edge_bps);
        env.storage()
            .instance()
            .set(&BET_MAP, &Map::<u64, Bet>::new(&env));
    }

    /// Place a bet. Transfers `amount` XLM from `player` into contract escrow.
    /// Returns the bet ID.
    pub fn place_bet(
        env: Env,
        player: Address,
        amount: i128,
        game_type: Symbol,
        prediction: u64,
    ) -> u64 {
        player.require_auth();

        if amount <= 0 {
            panic!("bet amount must be positive");
        }

        let token_address: Address = env.storage().instance().get(&XLM_TOKEN).unwrap();
        let token_client = token::Client::new(&env, &token_address);

        // Transfer funds from player to contract
        token_client.transfer(&player, &env.current_contract_address(), &amount);

        let bet_id = env.ledger().timestamp();
        let bet = Bet {
            player: player.clone(),
            amount,
            game_type: game_type.clone(),
            prediction,
            status: BetStatus::Pending,
            payout: 0,
            timestamp: env.ledger().timestamp(),
        };

        let mut bet_map: Map<u64, Bet> = env
            .storage()
            .instance()
            .get(&BET_MAP)
            .unwrap_or(Map::new(&env));
        bet_map.set(bet_id, bet);
        env.storage().instance().set(&BET_MAP, &bet_map);

        log!(&env, "place_bet: player={}, amount={}, bet_id={}", player, amount, bet_id);
        bet_id
    }

    /// Resolve a bet. Called by admin (or an authorized game contract) with the outcome.
    /// `outcome` is the actual result; compared against `Bet.prediction`.
    /// If the player won, payout = amount * multiplier minus house edge.
    pub fn resolve_bet(env: Env, bet_id: u64, outcome: u64, multiplier_bps: u32) {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        admin.require_auth();

        let mut bet_map: Map<u64, Bet> = env.storage().instance().get(&BET_MAP).unwrap();
        let mut bet = bet_map.get(bet_id).expect("bet not found");

        if bet.status != BetStatus::Pending {
            panic!("bet already resolved");
        }

        let house_edge_bps: u32 = env.storage().instance().get(&HOUSE_EDGE_BPS).unwrap();
        let token_address: Address = env.storage().instance().get(&XLM_TOKEN).unwrap();
        let token_client = token::Client::new(&env, &token_address);

        if bet.prediction == outcome {
            // Winner: payout = amount * multiplier_bps / 10000, minus house edge
            let gross_payout = (bet.amount * multiplier_bps as i128) / 10_000;
            let house_cut = (gross_payout * house_edge_bps as i128) / 10_000;
            let net_payout = gross_payout - house_cut;

            bet.status = BetStatus::Won;
            bet.payout = net_payout;

            token_client.transfer(&env.current_contract_address(), &bet.player, &net_payout);
            log!(&env, "resolve_bet: bet_id={} WON, payout={}", bet_id, net_payout);
        } else {
            bet.status = BetStatus::Lost;
            bet.payout = 0;
            log!(&env, "resolve_bet: bet_id={} LOST", bet_id);
        }

        bet_map.set(bet_id, bet);
        env.storage().instance().set(&BET_MAP, &bet_map);
    }

    /// Withdraw house earnings. Admin only.
    pub fn withdraw(env: Env, recipient: Address, amount: i128) {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        admin.require_auth();

        let token_address: Address = env.storage().instance().get(&XLM_TOKEN).unwrap();
        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&env.current_contract_address(), &recipient, &amount);

        log!(&env, "withdraw: amount={} to {}", amount, recipient);
    }

    /// Get the contract's current XLM balance.
    pub fn get_balance(env: Env) -> i128 {
        let token_address: Address = env.storage().instance().get(&XLM_TOKEN).unwrap();
        let token_client = token::Client::new(&env, &token_address);
        token_client.balance(&env.current_contract_address())
    }

    /// Get a specific bet by ID.
    pub fn get_bet(env: Env, bet_id: u64) -> Bet {
        let bet_map: Map<u64, Bet> = env.storage().instance().get(&BET_MAP).unwrap();
        bet_map.get(bet_id).expect("bet not found")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Ledger},
        token::{Client as TokenClient, StellarAssetClient},
        Address, Env,
    };

    fn setup() -> (Env, LumaBetCoreClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let player = Address::generate(&env);

        // Deploy a mock XLM token (Stellar Asset Contract)
        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let xlm_token = token_id.address();

        // Mint some XLM to the player
        let asset_client = StellarAssetClient::new(&env, &xlm_token);
        asset_client.mint(&player, &10_000_0000000); // 10,000 XLM (7 decimals)

        let contract_id = env.register(LumaBetCore, ());
        let client = LumaBetCoreClient::new(&env, &contract_id);

        // Initialize
        client.initialize(&admin, &xlm_token, &200u32); // 2% house edge

        (env, client, admin, player, xlm_token)
    }

    #[test]
    fn test_place_and_lose_bet() {
        let (_env, client, _admin, player, _xlm) = setup();

        let bet_id = client.place_bet(
            &player,
            &1_0000000i128, // 1 XLM
            &symbol_short!("DICE"),
            &3u64,
        );

        // Admin resolves with outcome = 5 (player predicted 3, so they lose)
        client.resolve_bet(&bet_id, &5u64, &20000u32); // 2x multiplier

        let bet = client.get_bet(&bet_id);
        assert_eq!(bet.status, BetStatus::Lost);
        assert_eq!(bet.payout, 0);
    }

    #[test]
    fn test_place_and_win_bet() {
        let (_env, client, _admin, player, _xlm) = setup();

        let bet_id = client.place_bet(
            &player,
            &1_0000000i128,
            &symbol_short!("DICE"),
            &4u64,
        );

        // Admin resolves with outcome = 4 (player wins), 6x multiplier
        client.resolve_bet(&bet_id, &4u64, &60000u32);

        let bet = client.get_bet(&bet_id);
        assert_eq!(bet.status, BetStatus::Won);
        assert!(bet.payout > 0);
    }

    #[test]
    #[should_panic(expected = "bet amount must be positive")]
    fn test_reject_zero_bet() {
        let (_env, client, _admin, player, _xlm) = setup();
        client.place_bet(&player, &0i128, &symbol_short!("DICE"), &1u64);
    }
}
