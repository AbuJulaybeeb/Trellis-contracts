use crate::logging::log_trade;
use crate::slippage_predictor::predict_slippage;
use crate::{ExecutionReport, ExecutionStrategy, Trade, TradeReceipt, TradeStatus};
use soroban_sdk::{Env, U256, Vec};

pub const ERROR_ZERO_AMOUNT: u32 = 1;
pub const ERROR_IDENTICAL_ASSETS: u32 = 2;
pub const ERROR_EXCEEDS_CAPACITY: u32 = 3;

pub fn execute_strategy(
    env: &Env,
    _strategy: &ExecutionStrategy,
    trades: &Vec<Trade>,
) -> ExecutionReport {
    let mut total_fees: u128 = 0;
    let mut total_slippage: u128 = 0;
    let mut trades_executed: u32 = 0;
    let mut trades_failed: u32 = 0;
    let mut trade_statuses = Vec::new(env);

    for trade in trades.iter() {
        if trade.amount == 0 {
            trades_failed += 1;
            trade_statuses.push_back(TradeReceipt {
                asset_pair: trade.asset_pair.clone(),
                amount: trade.amount,
                status: TradeStatus::Failed,
                fee: 0,
                error_code: Some(ERROR_ZERO_AMOUNT),
            });
            continue;
        }

        if trade.asset_pair.0 == trade.asset_pair.1 {
            trades_failed += 1;
            trade_statuses.push_back(TradeReceipt {
                asset_pair: trade.asset_pair.clone(),
                amount: trade.amount,
                status: TradeStatus::Failed,
                fee: 0,
                error_code: Some(ERROR_IDENTICAL_ASSETS),
            });
            continue;
        }

        if trade.amount > 10_000_000_000 {
            trades_failed += 1;
            trade_statuses.push_back(TradeReceipt {
                asset_pair: trade.asset_pair.clone(),
                amount: trade.amount,
                status: TradeStatus::Failed,
                fee: 0,
                error_code: Some(ERROR_EXCEEDS_CAPACITY),
            });
            continue;
        }

        let fee = trade.amount / 1000;
        total_fees = total_fees.saturating_add(fee);

        let slippage = predict_slippage(trade.asset_pair.clone(), trade.amount, env);
        let slippage_val = slippage.to_u128().unwrap_or(0);
        total_slippage = total_slippage.saturating_add(slippage_val);

        trades_executed += 1;

        // Mock execution price of 1_000_000
        let actual_price: u128 = 1_000_000;
        log_trade(env, &trade, actual_price, fee);

        trade_statuses.push_back(TradeReceipt {
            asset_pair: trade.asset_pair.clone(),
            amount: trade.amount,
            status: TradeStatus::Success,
            fee,
            error_code: None,
        });
    }

    let actual_slippage = U256::from_u128(env, total_slippage);

    ExecutionReport {
        total_fees,
        actual_slippage,
        trades_executed,
        trades_failed,
        trade_statuses,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, Env};

    #[test]
    fn test_execute_strategy_success() {
        let env = Env::default();
        let contract = env.register_contract(None, crate::MultiAssetRebalancer);

        let mut trades = Vec::new(&env);
        trades.push_back(Trade {
            asset_pair: (symbol_short!("USDC"), symbol_short!("XLM")),
            amount: 1000,
        });

        env.as_contract(&contract, || {
            let report = execute_strategy(&env, &ExecutionStrategy::Balanced, &trades);
            assert_eq!(report.trades_executed, 1);
            assert_eq!(report.trades_failed, 0);
            assert_eq!(report.total_fees, 1);
            assert_eq!(report.trade_statuses.len(), 1);
            assert_eq!(report.trade_statuses.get(0).unwrap().status, TradeStatus::Success);
        });
    }

    #[test]
    fn test_execute_strategy_with_failures() {
        let env = Env::default();
        let contract = env.register_contract(None, crate::MultiAssetRebalancer);

        let mut trades = Vec::new(&env);
        trades.push_back(Trade {
            asset_pair: (symbol_short!("USDC"), symbol_short!("XLM")),
            amount: 1000,
        });
        trades.push_back(Trade {
            asset_pair: (symbol_short!("USDC"), symbol_short!("USDC")),
            amount: 500,
        });
        trades.push_back(Trade {
            asset_pair: (symbol_short!("USDC"), symbol_short!("XLM")),
            amount: 0,
        });

        env.as_contract(&contract, || {
            let report = execute_strategy(&env, &ExecutionStrategy::Balanced, &trades);
            assert_eq!(report.trades_executed, 1);
            assert_eq!(report.trades_failed, 2);
            assert_eq!(report.trade_statuses.len(), 3);
            assert_eq!(report.trade_statuses.get(1).unwrap().error_code, Some(ERROR_IDENTICAL_ASSETS));
            assert_eq!(report.trade_statuses.get(2).unwrap().error_code, Some(ERROR_ZERO_AMOUNT));
        });
    }
}
