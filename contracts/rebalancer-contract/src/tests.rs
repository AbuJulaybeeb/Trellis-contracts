#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, Env};

#[test]
fn test_rebalance_dry_run() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MultiAssetRebalancer);
    let client = MultiAssetRebalancerClient::new(&env, &contract_id);

    let mut trades = Vec::new(&env);
    trades.push_back(Trade {
        asset_pair: (symbol_short!("USDC"), symbol_short!("XLM")),
        amount: 1000,
    });

    let result = client.rebalance(&trades, &ExecutionStrategy::Balanced, &true);

    assert_eq!(result.total_fees, 1);
    assert!(result.actual_slippage.to_u128().is_some());
    assert_eq!(result.trades_executed, 0);
    assert_eq!(result.trades_failed, 0);
    assert_eq!(result.trade_statuses.len(), 1);

    let receipt = result.trade_statuses.get(0).unwrap();
    assert_eq!(receipt.status, TradeStatus::Success);
    assert_eq!(receipt.fee, 1);
    assert_eq!(receipt.error_code, None);
}

#[test]
fn test_rebalance_accumulates_slippage_from_multiple_trades() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MultiAssetRebalancer);
    let client = MultiAssetRebalancerClient::new(&env, &contract_id);

    let mut trades = Vec::new(&env);
    trades.push_back(Trade {
        asset_pair: (symbol_short!("USDC"), symbol_short!("XLM")),
        amount: 1000,
    });
    trades.push_back(Trade {
        asset_pair: (symbol_short!("USDC"), symbol_short!("BTC")),
        amount: 500_000,
    });

    let result = client.rebalance(&trades, &ExecutionStrategy::Balanced, &true);

    assert!(result.actual_slippage.to_u128().unwrap_or(0) > 0, "Slippage should be accumulated");
    assert_eq!(result.trade_statuses.len(), 2);
}

#[test]
fn test_rebalance_zero_trade_slippage() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MultiAssetRebalancer);
    let client = MultiAssetRebalancerClient::new(&env, &contract_id);

    let mut trades = Vec::new(&env);
    trades.push_back(Trade {
        asset_pair: (symbol_short!("USDC"), symbol_short!("XLM")),
        amount: 0,
    });

    let result = client.rebalance(&trades, &ExecutionStrategy::Balanced, &true);

    assert_eq!(result.actual_slippage.to_u128().unwrap_or(0), 0, "Zero trade should have zero slippage");
    assert_eq!(result.trade_statuses.get(0).unwrap().status, TradeStatus::Failed);
}

#[test]
fn test_rebalance_live_execution_with_partial_failures() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MultiAssetRebalancer);
    let client = MultiAssetRebalancerClient::new(&env, &contract_id);

    let mut trades = Vec::new(&env);
    // Successful trade
    trades.push_back(Trade {
        asset_pair: (symbol_short!("USDC"), symbol_short!("XLM")),
        amount: 2000,
    });
    // Failed trade: zero amount
    trades.push_back(Trade {
        asset_pair: (symbol_short!("USDC"), symbol_short!("BTC")),
        amount: 0,
    });
    // Failed trade: identical assets
    trades.push_back(Trade {
        asset_pair: (symbol_short!("ETH"), symbol_short!("ETH")),
        amount: 500,
    });

    let result = client.rebalance(&trades, &ExecutionStrategy::Balanced, &false);

    assert_eq!(result.trades_executed, 1);
    assert_eq!(result.trades_failed, 2);
    assert_eq!(result.total_fees, 2);
    assert_eq!(result.trade_statuses.len(), 3);

    let receipt0 = result.trade_statuses.get(0).unwrap();
    assert_eq!(receipt0.status, TradeStatus::Success);
    assert_eq!(receipt0.fee, 2);
    assert_eq!(receipt0.error_code, None);

    let receipt1 = result.trade_statuses.get(1).unwrap();
    assert_eq!(receipt1.status, TradeStatus::Failed);
    assert_eq!(receipt1.error_code, Some(strategy_executor::ERROR_ZERO_AMOUNT));

    let receipt2 = result.trade_statuses.get(2).unwrap();
    assert_eq!(receipt2.status, TradeStatus::Failed);
    assert_eq!(receipt2.error_code, Some(strategy_executor::ERROR_IDENTICAL_ASSETS));
}