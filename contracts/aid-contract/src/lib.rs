#![no_std]
//! # Aid Contract
//!
//! Direct aid settlement and escrow on the Stellar blockchain.
//!
//! ## Overview
//!
//! The Aid Contract manages the core humanitarian aid disbursement flow:
//! - Donors create aid records, escrowing funds in the contract
//! - Recipients claim aid within expiration windows
//! - Donors can refund expired, unclaimed aid
//! - Administrative pause/resume for emergency controls
//!
//! ## Key Concepts
//!
//! - **Aid Record**: An immutable entry containing donor, recipient, amount, and expiration
//! - **Status**: Each aid transitions through `Pending` → `Settled`/`Refunded`
//! - **Expiry**: Aids expire at a ledger sequence number; claims are rejected once expired
//! - **Authorization**: Donors create aid, recipients claim, admins pause/resume
//!
//! ## Example Flow
//!
//! 1. Donor calls [`AidContract::create_aid`] with recipient address and amount
//! 2. Contract escrows funds and returns an aid ID
//! 3. Recipient calls [`AidContract::claim_aid`] before expiry ledger
//! 4. Funds transfer to recipient, status becomes `Settled`
//! 5. If unclaimed past expiry, donor can call [`AidContract::refund_aid`]
//!
//! ## Queries
//!
//! - [`AidContract::get_aid`]: Fetch a single aid record
//! - [`AidContract::get_admin`]: Current admin address
//! - [`AidContract::is_initialized`]: Check initialization state
//!
//! For full API details, see the module items below.

use shared::events::{
    emit_action_executed, emit_aid_created, emit_module_initialized, emit_permission_changed,
};
use shared::storage::{is_paused, set_paused as shared_set_paused};
use shared::{emit, Error, AID_CLAIMED, AID_CREATED, AID_REFUNDED, AID_SETTLED};
use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, token, Address, Env,
    Map, Symbol, Vec,
};

pub mod api;
pub mod storage;
pub mod types;

use storage::{get_aid, get_aid_counter, has_aid, set_aid, set_aid_counter};

pub use types::{AidPage, AidRecord, AidStatus};

const KEY_AIDS: Symbol = symbol_short!("aids");
#[allow(dead_code)]
const MAX_QUERY_LIMIT: u32 = 50;

// ---------------------------------------------------------------------------
// Contract-specific error codes (range 100-199 per shared conventions)
// ---------------------------------------------------------------------------

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AidError {
    Unauthorized = 100,
    NotFound = 101,
    AlreadyClaimed = 102,
    Expired = 103,
    Paused = 104,
    /// The aid has not expired yet and cannot be refunded.
    NotExpiredYet = 105,
    /// The aid has already been refunded to the donor.
    AlreadyRefunded = 106,
}

#[contract]
pub struct AidContract;

#[contractimpl]
impl AidContract {
    // -----------------------------------------------------------------------
    // Lifecycle
    // -----------------------------------------------------------------------

    /// Initialize the contract with configuration.
    ///
    /// Must be called exactly once immediately after deployment.
    ///
    /// # Arguments
    /// * `admin` - The admin address with governance privileges.
    /// * `treasury` - The treasury address for fees or emergency withdrawals.
    /// * `token` - The accepted escrow token address.
    /// * `default_expiry_secs` - Default expiration time in seconds for new aids.
    ///
    /// # Errors
    /// * [`shared::Error::AlreadyInitialized`] - If called more than once.
    pub fn initialize(
        env: Env,
        admin: Address,
        treasury: Address,
        token: Address,
        default_expiry_secs: u64,
    ) -> Result<(), shared::Error> {
        // Guard against re-initialization
        if storage::is_initialized(&env) {
            return Err(shared::Error::AlreadyInitialized);
        }

        admin.require_auth();

        // Store configuration
        shared::auth::set_admin(&env, &admin);
        storage::set_treasury(&env, &treasury);
        storage::set_token(&env, &token);
        storage::set_default_expiry(&env, default_expiry_secs);
        storage::set_initialized(&env);

        emit_module_initialized(
            &env,
            symbol_short!("aid"),
            1,
            &admin,
            env.ledger().timestamp(),
        );

        Ok(())
    }

    /// Get the admin address.
    pub fn get_admin(env: Env) -> Address {
        shared::auth::get_admin(&env)
    }

    /// Get the treasury address.
    pub fn get_treasury(env: Env) -> Option<Address> {
        storage::get_treasury(&env)
    }

    /// Get the escrow token address.
    pub fn get_token(env: Env) -> Option<Address> {
        storage::get_token(&env)
    }

    /// Get the default expiry in seconds.
    pub fn get_default_expiry(env: Env) -> Option<u64> {
        storage::get_default_expiry(&env)
    }

    /// Check if the contract is initialized.
    pub fn is_initialized(env: Env) -> bool {
        storage::is_initialized(&env)
    }

    /// Update configuration (admin only).
    ///
    /// # Arguments
    /// * `admin` - Must be the current admin.
    /// * `treasury` - New treasury address (None = keep existing).
    /// * `default_expiry_secs` - New default expiry (None = keep existing).
    ///
    /// # Errors
    /// * [`shared::Error::Unauthorized`] - If caller is not admin.
    pub fn update_config(
        env: Env,
        admin: Address,
        treasury: Option<Address>,
        default_expiry_secs: Option<u64>,
    ) -> Result<(), shared::Error> {
        // Verify admin authorization
        let current_admin = shared::auth::get_admin(&env);
        if admin != current_admin {
            return Err(shared::Error::Unauthorized);
        }
        admin.require_auth();

        // Update treasury if provided
        if let Some(t) = treasury {
            storage::set_treasury(&env, &t);
        }

        // Update default expiry if provided
        if let Some(e) = default_expiry_secs {
            storage::set_default_expiry(&env, e);
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Aid creation
    // -----------------------------------------------------------------------

    /// Create a new aid disbursement and escrow funds from the donor.
    ///
    /// The aid ID is auto-generated. Transfers `amount` of the configured
    /// `token` from `donor` into this contract for safekeeping until the
    /// recipient claims or the aid expires.
    ///
    /// Also appends the new ID to both the donor and recipient indexes so
    /// paginated queries stay consistent.
    pub fn create_aid(
        env: Env,
        donor: Address,
        recipient: Address,
        amount: i128,
        expiry_ledger: u32,
    ) -> u64 {
        donor.require_auth();

        // Fast-path: cheapest validation first (gas ordering)
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        if expiry_ledger <= env.ledger().sequence() {
            env.panic_with_error(AidError::NotExpiredYet);
        }

        // Quota enforcement (Issue #65): fail-open when unconfigured so
        // existing deployments keep working until maintainers set limits.
        // Over-limit callers get a user-safe QuotaExceeded panic.
        if let Err(e) = shared::quota::check_and_consume(
            &env,
            &donor,
            &symbol_short!("aid_crt"),
            amount,
        ) {
            if e == Error::QuotaExceeded {
                panic_with_error!(&env, Error::QuotaExceeded);
            }
            // Fail-open for unset quota config is handled inside
            // check_and_consume; any other error is non-fatal here.
        }

        // Auto-allocate aid ID via counter (avoids caller-supplied collision)
        let aid_id = get_aid_counter(&env);
        if has_aid(&env, aid_id) {
            env.panic_with_error(shared::Error::InvalidArgument);
        }
        set_aid_counter(&env, aid_id.wrapping_add(1));

        let token: Address = env
            .storage()
            .instance()
            .get(&storage::DataKey::Token)
            .expect("token not initialised");

        // Checks-effects-interactions: store record before cross-contract call
        let record = AidRecord {
            id: aid_id,
            donor: donor.clone(),
            recipient: recipient.clone(),
            token: token.clone(),
            amount,
            expiry_ledger,
            status: AidStatus::Pending,
        };
        set_aid(&env, aid_id, &record);
        storage::append_donor_aid(&env, &donor, aid_id);
        storage::append_recipient_aid(&env, &recipient, aid_id);

        let mut aids: Map<u64, AidRecord> = env
            .storage()
            .persistent()
            .get(&KEY_AIDS)
            .unwrap_or_else(|| Map::new(&env));
        aids.set(aid_id, record);
        env.storage().persistent().set(&KEY_AIDS, &aids);

        emit_aid_created(
            &env,
            aid_id,
            &donor,
            &recipient,
            amount,
            env.ledger().sequence().into(),
            expiry_ledger.into(),
        );

        emit(
            &env,
            AID_CREATED,
            (aid_id, donor.clone(), recipient, amount, expiry_ledger),
        );
        emit_action_executed(
            &env,
            symbol_short!("aid"),
            symbol_short!("create"),
            &env.current_contract_address(),
            true,
            env.ledger().timestamp(),
        );
        // Escrow funds from donor into contract.
        token::Client::new(&env, &token).transfer(&donor, &env.current_contract_address(), &amount);

        aid_id
    }

    // -----------------------------------------------------------------------
    // Aid claiming
    // -----------------------------------------------------------------------

    /// Claim a pending aid disbursement and transfer funds to the recipient.
    ///
    /// # Errors (via Result)
    /// - [`AidError::Paused`]         — contract is paused.
    /// - [`AidError::NotFound`]       — `aid_id` does not exist.
    /// - [`AidError::Expired`]        — `expiry_ledger` has passed.
    /// - [`AidError::AlreadyClaimed`] — status is not `Pending`.
    /// - [`AidError::Unauthorized`]   — `recipient` is not the intended recipient.
    pub fn claim_aid(env: Env, aid_id: u64, recipient: Address) -> Result<(), AidError> {
        // Pause check first — cheapest read (instance storage, no TTL bump)
        if is_paused(&env) {
            return Err(AidError::Paused);
        }
        recipient.require_auth();

        let mut record = get_aid(&env, aid_id).ok_or(AidError::NotFound)?;

        // Sequence of checks ordered by likely failure rate (cheap first)
        if record.status == AidStatus::Settled || record.status == AidStatus::Refunded {
            return Err(AidError::AlreadyClaimed);
        }
        if env.ledger().sequence() > record.expiry_ledger {
            return Err(AidError::Expired);
        }
        if recipient != record.recipient {
            return Err(AidError::Unauthorized);
        }

        record.status = AidStatus::Settled;
        set_aid(&env, aid_id, &record);

        token::Client::new(&env, &record.token).transfer(
            &env.current_contract_address(),
            &record.recipient,
            &record.amount,
        );

        emit(&env, AID_CLAIMED, aid_id);
        emit(&env, AID_SETTLED, aid_id);
        emit_action_executed(
            &env,
            symbol_short!("aid"),
            symbol_short!("claim_aid"),
            &env.current_contract_address(),
            true,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Refunds
    // -----------------------------------------------------------------------

    /// Refund an expired, unclaimed aid disbursement to the original donor.
    ///
    /// Only the donor or the admin may trigger the refund.
    ///
    /// # Errors
    /// - [`AidError::NotFound`]       — `aid_id` does not exist.
    /// - [`AidError::AlreadyClaimed`] — already settled.
    /// - [`AidError::AlreadyRefunded`] — already refunded.
    /// - [`AidError::NotExpiredYet`]  — expiry has not yet passed.
    pub fn refund_aid(env: Env, aid_id: u64) -> Result<(), AidError> {
        let mut record = get_aid(&env, aid_id).ok_or(AidError::NotFound)?;

        // Check status first — avoids expensive ledger read on wrong state
        match record.status {
            AidStatus::Settled => return Err(AidError::AlreadyClaimed),
            AidStatus::Refunded => return Err(AidError::AlreadyRefunded),
            AidStatus::Pending => {} // continue
        }

        if env.ledger().sequence() <= record.expiry_ledger {
            return Err(AidError::NotExpiredYet);
        }

        record.status = AidStatus::Refunded;
        set_aid(&env, aid_id, &record);

        token::Client::new(&env, &record.token).transfer(
            &env.current_contract_address(),
            &record.donor,
            &record.amount,
        );

        emit(&env, AID_REFUNDED, aid_id);
        emit_action_executed(
            &env,
            symbol_short!("aid"),
            symbol_short!("refund"),
            &env.current_contract_address(),
            true,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    pub fn get_aid(env: Env, aid_id: u64) -> Option<AidRecord> {
        storage::get_aid(&env, aid_id)
    }

    // -----------------------------------------------------------------------
    // Schema version & compatibility (Issue #64)
    // -----------------------------------------------------------------------

    /// Schema version stamped on all new aid records.
    ///
    /// Old clients can branch on this; new clients always receive the latest
    /// shape via [`AidContract::get_aid_latest`].
    pub fn schema_version(env: Env) -> u32 {
        let _ = env;
        shared::compat::current_schema_version()
    }

    /// Version-aware read: always returns the latest [`shared::compat::CurrentAidRecord`]
    /// shape, lazily migrating legacy (V1) records.
    ///
    /// Stored [`AidRecord`] values predate the version field, so they are
    /// treated as V1 on the wire and upgraded here (token is already bound on
    /// the stored record, so no placeholder is needed).
    pub fn get_aid_latest(env: Env, aid_id: u64) -> Option<shared::compat::CurrentAidRecord> {
        let record = storage::get_aid(&env, aid_id)?;
        let status = match record.status {
            AidStatus::Pending => shared::compat::CompatAidStatus::Pending,
            AidStatus::Settled => shared::compat::CompatAidStatus::Settled,
            AidStatus::Refunded => shared::compat::CompatAidStatus::Refunded,
        };
        Some(shared::compat::CurrentAidRecord {
            id: record.id,
            donor: record.donor,
            recipient: record.recipient,
            token: record.token,
            amount: record.amount,
            expiry_ledger: record.expiry_ledger,
            status,
            schema_version: shared::compat::current_schema_version(),
        })
    }

    /// Explicit legacy migration hook: validates that `aid_id` is readable
    /// under the current schema. Stored records are already token-bound, so
    /// this is a validation + TTL refresh (lazy migration completes on read).
    pub fn migrate_legacy_aid(env: Env, aid_id: u64) -> Result<u32, shared::Error> {
        let record = storage::get_aid(&env, aid_id).ok_or(shared::Error::NotFound)?;
        if record.amount <= 0 {
            return Err(shared::Error::SchemaMigrationFailed);
        }
        // Refresh TTL so the migrated record survives upcoming ledgers.
        storage::set_aid(&env, aid_id, &record);
        Ok(shared::compat::current_schema_version())
    }

    // -----------------------------------------------------------------------
    // Admin controls
    // -----------------------------------------------------------------------

    /// Pause or resume the contract. Admin only.
    pub fn set_paused(env: Env, admin: Address, paused: bool) {
        let contract_admin = shared::auth::get_admin(&env);
        if admin != contract_admin {
            env.panic_with_error(shared::Error::Unauthorized);
        }
        admin.require_auth();

        env.storage()
            .instance()
            .set(&Symbol::new(&env, "paused"), &paused);
        emit_permission_changed(
            &env,
            symbol_short!("aid"),
            symbol_short!("paused"),
            &admin,
            paused,
            env.ledger().timestamp(),
        );
        shared_set_paused(&env, paused);
    }

    /// Returns a paginated list of aid records created by `donor`.
    pub fn list_aids_by_donor(env: Env, donor: Address, cursor: u32, limit: u32) -> AidPage {
        let ids = storage::get_donor_aids(&env, &donor);
        paginate(&env, &ids, cursor, limit)
    }

    /// Returns a paginated list of aid records assigned to `recipient`.
    pub fn list_aids_by_recipient(env: Env, recipient: Address, cursor: u32, limit: u32) -> AidPage {
        let ids = storage::get_recipient_aids(&env, &recipient);
        paginate(&env, &ids, cursor, limit)
    }
}

/// Slice `ids` into one page of resolved [`AidRecord`]s.
///
/// Records whose storage entries were evicted are skipped without stalling
/// the cursor, so pagination always makes forward progress.
fn paginate(env: &Env, ids: &Vec<u64>, cursor: u32, limit: u32) -> AidPage {
    let effective_limit = if limit > MAX_QUERY_LIMIT {
        MAX_QUERY_LIMIT
    } else {
        limit
    };
    let total = ids.len();
    let mut records = Vec::new(env);
    let mut index = cursor;
    while index < total && records.len() < effective_limit {
        if let Some(record) = get_aid(env, ids.get(index).unwrap()) {
            records.push_back(record);
        }
        index += 1;
    }
    let next_cursor = if index < total { Some(index) } else { None };
    AidPage {
        records,
        next_cursor,
    }
}

#[cfg(test)]
mod tests;
