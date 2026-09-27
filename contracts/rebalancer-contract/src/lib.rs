#![no_std]

mod fee_calculator;
mod logging;
mod slippage_predictor;
pub mod strategy_executor;

pub use fee_calculator::calculate_total_fees;
pub use logging::log_trade;
use shared::events::emit_action_executed;
pub use slippage_predictor::predict_slippage;
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Env, Symbol, Vec, U256};
pub use strategy_executor::execute_strategy;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trade {
    pub asset_pair: (Symbol, Symbol),
    pub amount: u128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionStrategy {
    MinimalCost,
    MinimalTime,
    Balanced,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TradeStatus {
    Success = 1,
    PartialFill = 2,
    Failed = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TradeReceipt {
    pub asset_pair: (Symbol, Symbol),
    pub amount: u128,
    pub status: TradeStatus,
    pub fee: u128,
    pub error_code: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionReport {
    pub total_fees: u128,
    pub actual_slippage: U256,
    pub trades_executed: u32,
    pub trades_failed: u32,
    pub trade_statuses: Vec<TradeReceipt>,
}

impl ExecutionReport {
    pub fn trade_receipts(&self) -> &Vec<TradeReceipt> {
        &self.trade_statuses
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationResult {
    pub expected_fees: u128,
    pub expected_slippage: U256,
}

#[contract]
pub struct MultiAssetRebalancer;

#[contractimpl]
impl MultiAssetRebalancer {
    pub fn rebalance(
        env: Env,
        trades: Vec<Trade>,
        strategy: ExecutionStrategy,
        dry_run: bool,
    ) -> ExecutionReport {
        let report = if dry_run {
            let total_fees = calculate_total_fees(&trades);
            let mut total_slippage: u128 = 0;
            let mut trade_statuses = Vec::new(&env);

            for trade in trades.iter() {
                let slippage = predict_slippage(trade.asset_pair.clone(), trade.amount, &env);
                let slippage_val = slippage.to_u128().unwrap_or(0);
                total_slippage = total_slippage.saturating_add(slippage_val);

                let (status, fee, error_code) = if trade.amount == 0 {
                    (TradeStatus::Failed, 0, Some(strategy_executor::ERROR_ZERO_AMOUNT))
                } else if trade.asset_pair.0 == trade.asset_pair.1 {
                    (TradeStatus::Failed, 0, Some(strategy_executor::ERROR_IDENTICAL_ASSETS))
                } else if trade.amount > 10_000_000_000 {
                    (TradeStatus::Failed, 0, Some(strategy_executor::ERROR_EXCEEDS_CAPACITY))
                } else {
                    (TradeStatus::Success, trade.amount / 1000, None)
                };

                trade_statuses.push_back(TradeReceipt {
                    asset_pair: trade.asset_pair.clone(),
                    amount: trade.amount,
                    status,
                    fee,
                    error_code,
                });
            }

            let actual_slippage = U256::from_u128(&env, total_slippage);

            ExecutionReport {
                total_fees,
                actual_slippage,
                trades_executed: 0,
                trades_failed: 0,
                trade_statuses,
            }
        } else {
            execute_strategy(&env, &strategy, &trades)
        };

        emit_action_executed(
            &env,
            symbol_short!("reb"),
            symbol_short!("rebal"),
            &env.current_contract_address(),
            !dry_run,
            env.ledger().timestamp(),
        );

        report
    }
}

#[cfg(test)]
mod tests;
