use crate::{TreasuryContract, TreasuryContractClient};
use shared::errors::Error;
use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Events},
    Address, Env,
};

// ===========================================================================
// Test helpers
fn setup(env: &Env) -> (TreasuryContractClient<'static>, Address, i128) {
    let contract_id = env.register_contract(None, TreasuryContract);
    let client = TreasuryContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let limit: i128 = 1_000;
    client.initialize(&admin, &limit);
    (client, admin, limit)
}

// ===========================================================================
// Tests
#[test]
fn test_withdraw_success_decrements_balance_and_emits_event() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let category = symbol_short!("reserve");
    let recipient = Address::generate(&env);

    client.deposit(&admin, &token, &category, &500);

    client.withdraw(&admin, &token, &recipient, &200, &category);

    assert!(
        !env.events().all().is_empty(),
        "expected TREASURY_WITHDRAW event to be emitted"
    );
    assert_eq!(client.category_balance(&token, &category), 300);
}

// ===========================================================================
// Multi-token isolation tests
#[test]
fn test_multi_token_isolation_same_category() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token_usdc = Address::generate(&env);
    let token_xlm = Address::generate(&env);
    let category = symbol_short!("reserve");
    let recipient = Address::generate(&env);

    // Deposit 50,000 USDC and 100,000 XLM into the same category
    client.deposit(&admin, &token_usdc, &category, &50_000);
    client.deposit(&admin, &token_xlm, &category, &100_000);

    // Verify balances are stored independently
    assert_eq!(client.category_balance(&token_usdc, &category), 50_000);
    assert_eq!(client.category_balance(&token_xlm, &category), 100_000);

    // Withdrawing from USDC does NOT affect XLM balance
    client.withdraw(&admin, &token_usdc, &recipient, &500, &category);
    assert_eq!(client.category_balance(&token_usdc, &category), 49_500);
    assert_eq!(client.category_balance(&token_xlm, &category), 100_000);

    // Withdrawing from XLM does NOT affect USDC balance
    client.withdraw(&admin, &token_xlm, &recipient, &1_000, &category);
    assert_eq!(client.category_balance(&token_usdc, &category), 49_500);
    assert_eq!(client.category_balance(&token_xlm, &category), 99_000);

    // Attempting to withdraw Token C (not deposited) fails with InsufficientBalance
    let token_btc = Address::generate(&env);
    let result = client.try_withdraw(&admin, &token_btc, &recipient, &100, &category);
    assert_eq!(result, Err(Ok(Error::InsufficientBalance)));
}

// ===========================================================================
// Test: Withdraw Rejections
#[test]
fn test_withdraw_rejects_non_manager() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let category = symbol_short!("reserve");
    let recipient = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.deposit(&admin, &token, &category, &500);

    let result = client.try_withdraw(&stranger, &token, &recipient, &100, &category);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

// ===========================================================================
// Test: Withdraw Rejections
#[test]
fn test_withdraw_rejects_amount_above_limit() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, limit) = setup(&env);
    let token = Address::generate(&env);
    let category = symbol_short!("reserve");
    let recipient = Address::generate(&env);

    client.deposit(&admin, &token, &category, &(limit * 2));

    let over_limit = limit + 1;
    let result = client.try_withdraw(&admin, &token, &recipient, &over_limit, &category);
    assert_eq!(result, Err(Ok(Error::WithdrawalLimitExceeded)));
}

// ===========================================================================
// Test: Withdraw Rejections
#[test]
fn test_withdraw_rejects_insufficient_category_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let category = symbol_short!("rewards");
    let recipient = Address::generate(&env);

    client.deposit(&admin, &token, &category, &50);

    let result = client.try_withdraw(&admin, &token, &recipient, &100, &category);
    assert_eq!(result, Err(Ok(Error::InsufficientBalance)));
}

// ===========================================================================
// Test: Withdraw Rejections
#[test]
fn test_withdraw_rejects_zero_or_negative_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let category = symbol_short!("reserve");
    let recipient = Address::generate(&env);

    client.deposit(&admin, &token, &category, &500);

    let result = client.try_withdraw(&admin, &token, &recipient, &0, &category);
    assert_eq!(result, Err(Ok(Error::InvalidArgument)));
}

// ===========================================================================
// Test: Admin can add and remove treasury managers
#[test]
fn test_admin_can_add_and_remove_treasury_manager() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let category = symbol_short!("reserve");
    let recipient = Address::generate(&env);
    let manager = Address::generate(&env);

    client.deposit(&admin, &token, &category, &500);

    // Not yet a manager -> rejected.
    let result = client.try_withdraw(&manager, &token, &recipient, &100, &category);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    // Admin grants the role -> now allowed.
    client.add_treasury_manager(&admin, &manager);
    client.withdraw(&manager, &token, &recipient, &100, &category);
    assert_eq!(client.category_balance(&token, &category), 400);

    // Admin revokes the role -> rejected again.
    client.remove_treasury_manager(&admin, &manager);
    let result = client.try_withdraw(&manager, &token, &recipient, &50, &category);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

/// Stands in for the referral contract: it forwards to the treasury's
/// `distribute_reward`, so from the treasury's perspective the direct caller
/// is this contract's own address (whichever instance is registered).
#[contract]
struct MockReferralCaller;

#[contractimpl]
impl MockReferralCaller {
    pub fn call_distribute(
        env: Env,
        treasury: Address,
        token: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<(), Error> {
        let client = TreasuryContractClient::new(&env, &treasury);
        match client.try_distribute_reward(&token, &recipient, &amount) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(Error::InvalidArgument),
            Err(Ok(error)) => Err(error),
            Err(Err(_)) => Err(Error::InvalidArgument),
        }
    }
}

// ===========================================================================
// Test: Referral contract can call distribute_reward
#[test]
fn test_distribute_reward_pays_recipient_and_decrements_rewards() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let rewards = symbol_short!("rewards");
    let recipient = Address::generate(&env);
    let referral_id = env.register_contract(None, MockReferralCaller);
    let referral = MockReferralCallerClient::new(&env, &referral_id);

    client.deposit(&admin, &token, &rewards, &1_000);
    client.set_referral_contract(&admin, &referral_id);

    referral.call_distribute(&client.address, &token, &recipient, &400);

    assert!(
        !env.events().all().is_empty(),
        "expected CommissionPaid event to be emitted"
    );
    assert_eq!(client.category_balance(&token, &rewards), 600);
}

// ===========================================================================
// Test: Referral contract can call distribute_reward
#[test]
fn test_distribute_reward_rejects_underfunded_rewards_pool() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let rewards = symbol_short!("rewards");
    let recipient = Address::generate(&env);
    let referral_id = env.register_contract(None, MockReferralCaller);
    let referral = MockReferralCallerClient::new(&env, &referral_id);

    client.deposit(&admin, &token, &rewards, &100);
    client.set_referral_contract(&admin, &referral_id);

    let result = referral.try_call_distribute(&client.address, &token, &recipient, &200);
    assert_eq!(result, Err(Ok(Error::InsufficientBalance)));
    assert_eq!(client.category_balance(&token, &rewards), 100);
}

#[test]
fn test_distribute_reward_rejects_caller_that_is_not_registered_referral_contract() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let rewards = symbol_short!("rewards");
    let recipient = Address::generate(&env);
    let referral_id = env.register_contract(None, MockReferralCaller);
    let impostor_id = env.register_contract(None, MockReferralCaller);
    let impostor = MockReferralCallerClient::new(&env, &impostor_id);

    client.deposit(&admin, &token, &rewards, &1_000);
    client.set_referral_contract(&admin, &referral_id);

    // Disable blanket auth mocking
    env.set_auths(&[]);

    let result = impostor.try_call_distribute(&client.address, &token, &recipient, &400);
    assert!(result.is_err());
    assert_eq!(client.category_balance(&token, &rewards), 1_000);
}

#[test]
fn test_distribute_reward_rejects_when_no_referral_contract_registered() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let rewards = symbol_short!("rewards");
    let recipient = Address::generate(&env);
    let referral_id = env.register_contract(None, MockReferralCaller);
    let referral = MockReferralCallerClient::new(&env, &referral_id);

    client.deposit(&admin, &token, &rewards, &1_000);

    let result = referral.try_call_distribute(&client.address, &token, &recipient, &400);
    assert!(result.is_err());
    assert_eq!(client.category_balance(&token, &rewards), 1_000);
}

// ===========================================================================
// Gas benchmark tests
// ===========================================================================

#[test]
fn gas_bench_withdraw_rejects_zero_before_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let category = symbol_short!("reserve");
    let recipient = Address::generate(&env);

    client.deposit(&admin, &token, &category, &500);

    let stranger = Address::generate(&env);
    let result = client.try_withdraw(&stranger, &token, &recipient, &0, &category);
    assert_eq!(result, Err(Ok(Error::InvalidArgument)));
}

#[test]
fn gas_bench_deposit_rejects_zero_before_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let category = symbol_short!("reserve");
    let stranger = Address::generate(&env);

    let result = client.try_deposit(&stranger, &token, &category, &0);
    assert_eq!(result, Err(Ok(Error::InvalidArgument)));
}

#[test]
fn gas_bench_distribute_reward_rejects_zero_before_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let rewards = symbol_short!("rewards");
    let recipient = Address::generate(&env);

    client.deposit(&admin, &token, &rewards, &1_000);

    let result = client.try_distribute_reward(&token, &recipient, &0);
    assert_eq!(result, Err(Ok(Error::InvalidArgument)));
}

#[test]
fn gas_bench_emergency_withdraw_rejects_zero_before_auth() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _limit) = setup(&env);
    let token = Address::generate(&env);
    let recipient = Address::generate(&env);

    let result = client.try_emergency_withdraw(&admin, &token, &recipient, &0);
    assert_eq!(result, Err(Ok(Error::InvalidArgument)));
}
