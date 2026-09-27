use soroban_sdk::{Env, Symbol, U256};

pub fn predict_slippage(
    _asset_pair: (Symbol, Symbol),
    amount: u128,
    env: &Env,
) -> U256 {
    if amount == 0 {
        return U256::from_u32(env, 0);
    }
    let bps = (amount / 100_000).max(1) as u32;
    U256::from_u32(env, bps)
}
