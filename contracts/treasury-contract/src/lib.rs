#![no_std]
//! # Treasury Contract
//!
//! Protocol treasury management for Trellis humanitarian aid platform.
//!
//! ## Overview
//!
//! The Treasury Contract is the financial heart of Trellis, managing:
//! - **Multi-token per-category balances** (e.g., `reserve`, `rewards`) for fund segregation
//! - **Withdrawal limits** to prevent accidental large transfers
//! - **Role-based access control** via treasury managers and administrators
//! - **Referral reward distribution** through integration with the referral contract
//!
//! ## Categories
//!
//! The treasury organizes funds into categories per asset, each with its own balance:
//! - **`reserve`**: Protocol emergency funds
//! - **`rewards`**: Referral commission pool
//! - Custom categories as defined by administrators
//!
//! ## Roles
//!
//! - **Admin**: Full governance (set managers, configure limits, emergency withdraw)
//! - **Treasury Manager**: Operational access (deposit, withdraw, distribute rewards)
//!
//! ## Queries
//!
//! - [`TreasuryContract::category_balance`]: Check an asset category's current balance
//! - [`TreasuryContract::withdrawal_limit`]: View the max per-transaction limit
//! - [`TreasuryContract::referral_contract`]: See the registered referral contract

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

use shared::auth::{self, Role};
use shared::errors::Error;
use shared::events::{
    self, emit_action_executed, emit_commission_paid, emit_module_initialized,
    emit_permission_changed, emit_treasury_deposit, emit_treasury_withdrawal,
};
use shared::storage::{instance_get, instance_set, persistent_set};

/// Storage key prefix for per-category multi-token balances; the full key is
/// `(BALANCE, token, category)`.
const BALANCE: Symbol = symbol_short!("cat_bal");
/// Storage key for the configurable max per-transaction withdrawal limit.
const MAX_WD: Symbol = symbol_short!("max_wd");
/// Category symbol for the emergency reserve (used by `emergency_withdraw`).
const RESERVE_CATEGORY: Symbol = symbol_short!("reserve");
/// Category symbol for the referral commission rewards pool (used by
/// `distribute_reward`).
const REWARDS_CATEGORY: Symbol = symbol_short!("rewards");
/// Storage key for the referral contract address authorised to call
/// `distribute_reward`.
const REFERRAL_CONTRACT: Symbol = symbol_short!("ref_ctr");

#[contract]
pub struct TreasuryContract;

#[contractimpl]
impl TreasuryContract {
    /// Initialise the contract: sets the admin address, grants the admin
    /// the `TreasuryManager` role, and sets the initial max
    /// per-transaction withdrawal limit.
    pub fn initialize(env: Env, admin: Address, max_withdrawal_limit: i128) -> Result<(), Error> {
        if max_withdrawal_limit <= 0 {
            return Err(Error::InvalidArgument);
        }
        auth::set_admin(&env, &admin);
        persistent_set(
            &env,
            &shared::auth::DataKey::Role(admin.clone(), Role::TreasuryManager),
            &true,
        );
        instance_set(&env, &MAX_WD, &max_withdrawal_limit);
        emit_module_initialized(
            &env,
            symbol_short!("treasury"),
            1,
            &admin,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    /// Grants the `TreasuryManager` role to `who`. Admin only.
    pub fn add_treasury_manager(env: Env, caller: Address, who: Address) -> Result<(), Error> {
        auth::grant_role(&env, &caller, &who, Role::TreasuryManager)?;
        emit_permission_changed(
            &env,
            symbol_short!("treasury"),
            symbol_short!("manager"),
            &who,
            true,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    /// Revokes the `TreasuryManager` role from `who`. Admin only.
    pub fn remove_treasury_manager(env: Env, caller: Address, who: Address) -> Result<(), Error> {
        auth::revoke_role(&env, &caller, &who, Role::TreasuryManager)?;
        emit_permission_changed(
            &env,
            symbol_short!("treasury"),
            symbol_short!("manager"),
            &who,
            false,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    /// Updates the max per-transaction withdrawal limit. Admin only.
    pub fn set_withdrawal_limit(env: Env, caller: Address, new_limit: i128) -> Result<(), Error> {
        auth::require_admin(&env, &caller)?;
        if new_limit <= 0 {
            return Err(Error::InvalidArgument);
        }
        instance_set(&env, &MAX_WD, &new_limit);
        emit_action_executed(
            &env,
            symbol_short!("treasury"),
            symbol_short!("wd_limit"),
            &caller,
            true,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    /// Credits `amount` into `category`'s balance for `token`. `TreasuryManager` only.
    pub fn deposit(
        env: Env,
        caller: Address,
        token: Address,
        category: Symbol,
        amount: i128,
    ) -> Result<(), Error> {
        // Cheap validation first — avoids auth commit on trivial rejects.
        if amount <= 0 {
            return Err(Error::InvalidArgument);
        }
        auth::require_role(&env, &caller, Role::TreasuryManager)?;
        let key = (BALANCE, token.clone(), category.clone());
        let balance: i128 = env.storage().instance().get(&key).unwrap_or(0);
        let new_balance = balance.checked_add(amount).ok_or(Error::Overflow)?;
        env.storage().instance().set(&key, &new_balance);
        emit_treasury_deposit(&env, category, &caller, &token, amount, new_balance);
        emit_action_executed(
            &env,
            symbol_short!("treasury"),
            symbol_short!("deposit"),
            &caller,
            true,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    /// Returns the current balance for `token` in `category` (0 if never funded).
    pub fn category_balance(env: Env, token: Address, category: Symbol) -> i128 {
        instance_get::<_, i128>(&env, &(BALANCE, token, category)).unwrap_or(0)
    }

    /// Returns the currently configured max per-transaction withdrawal limit.
    pub fn withdrawal_limit(env: Env) -> i128 {
        instance_get::<_, i128>(&env, &MAX_WD).unwrap_or(0)
    }

    /// Withdraws `amount` of `token` from `category` to `to`.
    ///
    /// Guards, in order:
    /// 1. `amount` must be > 0                           → `Error::InvalidArgument`
    /// 2. `amount` must not exceed the withdrawal limit  → `Error::WithdrawalLimitExceeded`
    /// 3. `amount` must not exceed the category balance  → `Error::InsufficientBalance`
    /// 4. `caller` must hold `TreasuryManager`          → `Error::Unauthorized`
    ///
    /// On success, decrements the category balance and emits the shared
    /// `TreasuryWithdrawal` event with `(category, to, token, amount, remaining)`.
    pub fn withdraw(
        env: Env,
        caller: Address,
        token: Address,
        to: Address,
        amount: i128,
        category: Symbol,
    ) -> Result<(), Error> {
        // Gas optimization: cheap validation checks first
        if amount <= 0 {
            return Err(Error::InvalidArgument);
        }

        let limit: i128 = instance_get(&env, &MAX_WD).unwrap_or(0);
        if amount > limit {
            return Err(Error::WithdrawalLimitExceeded);
        }

        let key = (BALANCE, token.clone(), category.clone());
        let balance: i128 = instance_get(&env, &key).unwrap_or(0);
        if amount > balance {
            return Err(Error::InsufficientBalance);
        }

        // Auth check last
        auth::require_role(&env, &caller, Role::TreasuryManager)?;

        // Quota enforcement: fail-open when unconfigured.
        shared::quota::check_and_consume(&env, &caller, &symbol_short!("wdraw"), amount)?;

        let remaining = balance - amount;
        instance_set(&env, &key, &remaining);

        emit_treasury_withdrawal(&env, category, &to, &token, amount, remaining);
        emit_action_executed(
            &env,
            symbol_short!("treasury"),
            symbol_short!("withdraw"),
            &caller,
            true,
            env.ledger().timestamp(),
        );

        Ok(())
    }

    /// Emergency reserve withdrawal for `token`.
    ///
    /// Only callable by the admin, and only while the contract is paused.
    /// Intended to move reserve funds to safety when something has gone wrong.
    pub fn emergency_withdraw(
        env: Env,
        caller: Address,
        token: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error> {
        // Gas optimization: cheapest validations first.
        if amount <= 0 {
            return Err(Error::InvalidArgument);
        }
        if !shared::storage::is_paused(&env) {
            return Err(Error::NotPaused);
        }

        let key = (BALANCE, token.clone(), RESERVE_CATEGORY);
        let balance: i128 = instance_get(&env, &key).unwrap_or(0);
        let new_balance = balance
            .checked_sub(amount)
            .ok_or(Error::InsufficientBalance)?;
        if new_balance < 0 {
            return Err(Error::InsufficientBalance);
        }

        // Auth check last
        auth::require_admin(&env, &caller)?;
        instance_set(&env, &key, &new_balance);

        events::emit(
            &env,
            events::TREASURY_EMERGENCY_WITHDRAW,
            (caller.clone(), token, to, amount),
        );
        emit_action_executed(
            &env,
            symbol_short!("treasury"),
            symbol_short!("emrg_wd"),
            &caller,
            true,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    /// Registers the referral contract address authorised to call
    /// `distribute_reward`. Admin only.
    pub fn set_referral_contract(
        env: Env,
        caller: Address,
        referral_contract: Address,
    ) -> Result<(), Error> {
        auth::require_admin(&env, &caller)?;
        instance_set(&env, &REFERRAL_CONTRACT, &referral_contract);
        emit_action_executed(
            &env,
            symbol_short!("treasury"),
            symbol_short!("ref_ctr"),
            &caller,
            true,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    /// Returns the currently registered referral contract address, if any.
    pub fn referral_contract(env: Env) -> Option<Address> {
        instance_get(&env, &REFERRAL_CONTRACT)
    }

    /// Pays a referral commission of `amount` in `token` to `recipient` from the
    /// `Rewards` category.
    ///
    /// Callable only by the registered referral contract.
    pub fn distribute_reward(
        env: Env,
        token: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<(), Error> {
        // Cheap validation before auth commit.
        if amount <= 0 {
            return Err(Error::InvalidArgument);
        }

        let key = (BALANCE, token.clone(), REWARDS_CATEGORY);
        let balance: i128 = instance_get(&env, &key).unwrap_or(0);
        if amount > balance {
            return Err(Error::InsufficientBalance);
        }

        // Auth check after cheap validations pass.
        let referral_contract: Address =
            instance_get(&env, &REFERRAL_CONTRACT).ok_or(Error::Unauthorized)?;
        referral_contract.require_auth();

        let remaining = balance - amount;
        instance_set(&env, &key, &remaining);

        emit_commission_paid(&env, &recipient, &token, amount, env.ledger().timestamp());
        emit_action_executed(
            &env,
            symbol_short!("treasury"),
            symbol_short!("reward"),
            &recipient,
            true,
            env.ledger().timestamp(),
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Quota management (Issue #65) — maintainer diagnostics & overrides
    // -----------------------------------------------------------------------

    /// Set quota limits for a resource (admin only).
    pub fn set_quota_config(
        env: Env,
        caller: Address,
        resource: Symbol,
        config: shared::quota::QuotaConfig,
    ) -> Result<(), Error> {
        auth::require_admin(&env, &caller)?;
        shared::quota::set_quota_config(&env, &resource, &config)
    }

    /// Inspect quota usage for an actor/resource pair (maintainer diagnostics).
    pub fn quota_status(
        env: Env,
        actor: Address,
        resource: Symbol,
    ) -> shared::quota::QuotaStatus {
        shared::quota::get_quota_status(&env, &actor, &resource)
    }

    /// Reset quota usage for an actor/resource pair (admin override path).
    pub fn reset_quota(
        env: Env,
        caller: Address,
        actor: Address,
        resource: Symbol,
    ) -> Result<(), Error> {
        auth::require_admin(&env, &caller)?;
        shared::quota::reset_quota(&env, &actor, &resource);
        Ok(())
    }
}

#[cfg(test)]
mod test;
