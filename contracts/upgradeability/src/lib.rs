#![no_std]

//! # Upgradeability Module
//!
//! Provides a system-wide upgrade registry and coordinator for the Trellis
//! Soroban smart-contract suite.
//!
//! ## Design
//!
//! The module uses a **registry + coordinator** pattern adapted for Soroban:
//!
//! 1. **Registry** — tracks every upgradeable contract, its current version,
//!    WASM hash, and metadata.
//! 2. **Coordinator** — orchestrates upgrades: authorization checks, migration
//!    hook invocation, version bumping, and audit trail emission.
//! 3. **Migration hooks** — optional helper contracts that run pre/post
//!    upgrade logic (e.g., state transformations).
//!
//! Because Soroban does not expose EVM-style `delegatecall`, each upgradeable
//! contract exposes its own `upgrade` entry point.  The UpgradeabilityContract
//! validates the upgrade request (role + registry) and the target contract
//! calls `env.deployer().update_current_contract_wasm()`.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    IntoVal, Symbol, Val, Vec,
};

use shared::auth::{self, Role};
use shared::errors::Error;
use shared::events;
use shared::storage::{instance_get, instance_has, instance_remove, instance_set, persistent_set};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

#[allow(dead_code)]
const MAX_VERSION_NAME_LEN: usize = 64;
#[allow(dead_code)]
const MAX_MIGRATION_NOTE_LEN: usize = 256;

// ---------------------------------------------------------------------------
// Error codes — extend the shared error space for upgradeability
// ---------------------------------------------------------------------------

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum UpgradeError {
    /// The target contract is not registered in the upgrade registry.
    ContractNotRegistered = 900,
    /// A contract with the given name is already registered.
    ContractAlreadyRegistered = 901,
    /// The proposed WASM hash matches the current one (no-op upgrade).
    NoChangeDetected = 902,
    /// The upgrade proposal was not found.
    ProposalNotFound = 903,
    /// The upgrade proposal has already been executed.
    AlreadyExecuted = 904,
    /// The migration hook contract call failed.
    MigrationHookFailed = 905,
    /// The contract is already pending an upgrade.
    AlreadyPending = 906,
    /// The caller does not hold the Upgrader role.
    NotUpgrader = 907,
    /// The WASM hash is empty or invalid.
    InvalidWasmHash = 908,
    /// Storage layout incompatibility detected during migration.
    StorageIncompatible = 909,
    /// The migration hook address is not a valid contract.
    InvalidMigrationHook = 910,
}

type ContractResult<T> = core::result::Result<T, UpgradeError>;

// ---------------------------------------------------------------------------
// Storage key symbols (all <= 9 chars for symbol_short!)
// ---------------------------------------------------------------------------

const KEY_REG_CNT: Symbol = symbol_short!("reg_cnt");
const KEY_REG_ENTRY: Symbol = symbol_short!("reg_ent");
const KEY_PROP_CNT: Symbol = symbol_short!("upg_cnt");
const KEY_UPG_PROP: Symbol = symbol_short!("upg_prp");
const KEY_HOOK: Symbol = symbol_short!("mig_hook");
const KEY_HISTORY: Symbol = symbol_short!("upg_hist");
const KEY_PENDING: Symbol = symbol_short!("upg_pend");
const KEY_CONTRACT_BY_NAME: Symbol = symbol_short!("crt_name");

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Version information for a registered contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionInfo {
    /// Monotonically increasing version number (starts at 1).
    pub version: u32,
    /// The WASM hash of the currently active code.
    pub wasm_hash: BytesN<32>,
    /// Timestamp when this version was deployed.
    pub deployed_at: u64,
    /// Human-readable description of this version (optional).
    pub description: soroban_sdk::String,
}

/// Entry in the upgrade registry for a single contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryEntry {
    /// The contract's on-chain address.
    pub contract_id: Address,
    /// Logical name for the contract (e.g., "aid", "treasury").
    pub name: Symbol,
    /// Current active version.
    pub current: VersionInfo,
    /// Address of the migration hook contract (if set).
    pub migration_hook: Option<Address>,
}

/// An upgrade proposal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeProposal {
    /// Unique proposal ID.
    pub id: u64,
    /// The target contract address.
    pub contract_id: Address,
    /// The new WASM hash to deploy.
    pub new_wasm_hash: BytesN<32>,
    /// The new version number.
    pub new_version: u32,
    /// Optional migration note.
    pub note: soroban_sdk::String,
    /// Proposer address.
    pub proposer: Address,
    /// Whether this proposal has been executed.
    pub executed: bool,
    /// Timestamp when the proposal was created.
    pub created_at: u64,
    /// Timestamp when the proposal was executed (0 if pending).
    pub executed_at: u64,
}

/// A record in the upgrade history.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeRecord {
    /// The contract that was upgraded.
    pub contract_id: Address,
    /// Old version number.
    pub old_version: u32,
    /// New version number.
    pub new_version: u32,
    /// Old WASM hash.
    pub old_wasm_hash: BytesN<32>,
    /// New WASM hash.
    pub new_wasm_hash: BytesN<32>,
    /// Who executed the upgrade.
    pub executor: Address,
    /// When the upgrade was executed.
    pub executed_at: u64,
    /// Migration note.
    pub note: soroban_sdk::String,
}

/// The status of an upgrade for a contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpgradeStatus {
    /// No upgrade pending.
    Current,
    /// An upgrade proposal has been created but not yet executed.
    Pending(u64),
    /// The upgrade has been executed.
    Completed,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct UpgradeabilityContract;

#[contractimpl]
impl UpgradeabilityContract {
    /// Initialise the upgrade registry.
    ///
    /// Sets the admin address and grants the `Upgrader` role to the caller.
    pub fn initialize(env: Env, admin: Address) -> Result<(), UpgradeError> {
        shared::auth::set_admin(&env, &admin);
        // Grant the Upgrader role to admin so they can perform upgrades.
        persistent_set(
            &env,
            &auth::DataKey::Role(admin.clone(), Role::Upgrader),
            &true,
        );
        // Also grant Admin role for registry management.
        persistent_set(
            &env,
            &auth::DataKey::Role(admin.clone(), Role::Admin),
            &true,
        );

        events::emit_module_initialized(
            &env,
            symbol_short!("upg_reg"),
            1,
            &admin,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Contract registration
    // -----------------------------------------------------------------------

    /// Register a contract for upgrade management.
    ///
    /// Only callable by an address holding the `Admin` role.
    ///
    /// # Arguments
    /// * `caller` — Must hold the Admin role.
    /// * `contract_id` — The on-chain address of the contract to register.
    /// * `name` — A logical name (e.g., `symbol_short!("aid")`).
    /// * `version` — The initial version number (must be >= 1).
    /// * `wasm_hash` — The WASM hash of the initial deployment.
    pub fn register_contract(
        env: Env,
        caller: Address,
        contract_id: Address,
        name: Symbol,
        version: u32,
        wasm_hash: BytesN<32>,
    ) -> Result<(), UpgradeError> {
        require_admin_role(&env, &caller)?;

        if version < 1 {
            return Err(UpgradeError::InvalidWasmHash);
        }

        // Check no duplicate by name.
        if instance_has(&env, &(KEY_CONTRACT_BY_NAME, name.clone())) {
            return Err(UpgradeError::ContractAlreadyRegistered);
        }

        let entry = RegistryEntry {
            contract_id: contract_id.clone(),
            name: name.clone(),
            current: VersionInfo {
                version,
                wasm_hash: wasm_hash.clone(),
                deployed_at: env.ledger().timestamp(),
                description: soroban_sdk::String::from_str(&env, "initial"),
            },
            migration_hook: None,
        };

        instance_set(&env, &(KEY_REG_ENTRY, contract_id.clone()), &entry);
        instance_set(&env, &(KEY_CONTRACT_BY_NAME, name.clone()), &contract_id);

        // Update registration counter.
        let count: u64 = instance_get(&env, &KEY_REG_CNT).unwrap_or(0);
        instance_set(&env, &KEY_REG_CNT, &(count + 1));

        events::emit_contract_registered(
            &env,
            &contract_id,
            name,
            version,
            &wasm_hash,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    /// Returns the registry entry for a contract.
    pub fn get_registry_entry(
        env: Env,
        contract_id: Address,
    ) -> Result<RegistryEntry, UpgradeError> {
        instance_get(&env, &(KEY_REG_ENTRY, contract_id)).ok_or(UpgradeError::ContractNotRegistered)
    }

    /// Returns the registry entry by logical name.
    pub fn get_registry_entry_by_name(
        env: Env,
        name: Symbol,
    ) -> Result<RegistryEntry, UpgradeError> {
        let contract_id: Address = instance_get(&env, &(KEY_CONTRACT_BY_NAME, name))
            .ok_or(UpgradeError::ContractNotRegistered)?;
        instance_get(&env, &(KEY_REG_ENTRY, contract_id)).ok_or(UpgradeError::ContractNotRegistered)
    }

    /// Returns the total number of registered contracts.
    pub fn get_registered_count(env: Env) -> u64 {
        instance_get(&env, &KEY_REG_CNT).unwrap_or(0)
    }

    /// Returns the current version for a registered contract.
    pub fn get_version(env: Env, contract_id: Address) -> Result<u32, UpgradeError> {
        let entry: RegistryEntry = instance_get(&env, &(KEY_REG_ENTRY, contract_id))
            .ok_or(UpgradeError::ContractNotRegistered)?;
        Ok(entry.current.version)
    }

    /// Returns the current WASM hash for a registered contract.
    pub fn get_wasm_hash(env: Env, contract_id: Address) -> Result<BytesN<32>, UpgradeError> {
        let entry: RegistryEntry = instance_get(&env, &(KEY_REG_ENTRY, contract_id))
            .ok_or(UpgradeError::ContractNotRegistered)?;
        Ok(entry.current.wasm_hash)
    }

    // -----------------------------------------------------------------------
    // Migration hooks
    // -----------------------------------------------------------------------

    /// Register a migration hook contract for a registered contract.
    ///
    /// The hook contract must implement:
    /// * `pre_upgrade(env, old_version, new_version) -> bool`
    /// * `post_upgrade(env, old_version, new_version)`
    ///
    /// Only callable by an admin.
    pub fn set_migration_hook(
        env: Env,
        caller: Address,
        contract_id: Address,
        hook_addr: Address,
    ) -> Result<(), UpgradeError> {
        require_admin_role(&env, &caller)?;

        let mut entry: RegistryEntry = instance_get(&env, &(KEY_REG_ENTRY, contract_id.clone()))
            .ok_or(UpgradeError::ContractNotRegistered)?;

        entry.migration_hook = Some(hook_addr.clone());
        instance_set(&env, &(KEY_REG_ENTRY, contract_id.clone()), &entry);

        // Also store under a separate key for easy lookup.
        instance_set(&env, &(KEY_HOOK, contract_id.clone()), &hook_addr);

        events::emit_migration_hook_set(&env, &contract_id, &hook_addr, env.ledger().timestamp());
        Ok(())
    }

    /// Returns the migration hook address for a contract, if set.
    pub fn get_migration_hook(env: Env, contract_id: Address) -> Option<Address> {
        instance_get(&env, &(KEY_HOOK, contract_id))
    }

    // -----------------------------------------------------------------------
    // Upgrade proposals
    // -----------------------------------------------------------------------

    /// Create an upgrade proposal.
    ///
    /// Only callable by an address holding the `Upgrader` role.
    ///
    /// # Arguments
    /// * `caller` — Must hold the Upgrader role.
    /// * `contract_id` — The target contract to upgrade.
    /// * `new_wasm_hash` — The WASM hash of the new implementation.
    /// * `new_version` — The new version number (must be > current).
    /// * `note` — Optional migration note.
    pub fn propose_upgrade(
        env: Env,
        caller: Address,
        contract_id: Address,
        new_wasm_hash: BytesN<32>,
        new_version: u32,
        note: soroban_sdk::String,
    ) -> Result<u64, UpgradeError> {
        require_upgrader_role(&env, &caller)?;

        let entry: RegistryEntry = instance_get(&env, &(KEY_REG_ENTRY, contract_id.clone()))
            .ok_or(UpgradeError::ContractNotRegistered)?;

        // Validate: new version must be greater than current.
        if new_version <= entry.current.version {
            return Err(UpgradeError::NoChangeDetected);
        }

        // Validate: WASM hash must differ from current.
        if new_wasm_hash == entry.current.wasm_hash {
            return Err(UpgradeError::NoChangeDetected);
        }

        // Check no pending upgrade already exists.
        if instance_has(&env, &(KEY_PENDING, contract_id.clone())) {
            return Err(UpgradeError::AlreadyPending);
        }

        // Dry-run validate storage layout and protocol invariants if migration hook is set
        if let Some(ref hook_addr) = entry.migration_hook {
            execute_validate_storage_hook(&env, hook_addr, &contract_id)?;
        }

        // Create proposal.
        let proposal_id: u64 = instance_get(&env, &KEY_PROP_CNT).unwrap_or(0) + 1;
        instance_set(&env, &KEY_PROP_CNT, &proposal_id);

        let proposal = UpgradeProposal {
            id: proposal_id,
            contract_id: contract_id.clone(),
            new_wasm_hash: new_wasm_hash.clone(),
            new_version,
            note: note.clone(),
            proposer: caller.clone(),
            executed: false,
            created_at: env.ledger().timestamp(),
            executed_at: 0,
        };

        instance_set(&env, &(KEY_UPG_PROP, proposal_id), &proposal);
        instance_set(&env, &(KEY_PENDING, contract_id.clone()), &proposal_id);

        events::emit_upgrade_proposed(
            &env,
            proposal_id,
            &contract_id,
            new_version,
            &caller,
            env.ledger().timestamp(),
        );
        Ok(proposal_id)
    }

    /// Execute an upgrade proposal.
    ///
    /// This validates the migration hook (if present), updates the registry,
    /// and records the upgrade in the history.  The actual WASM replacement
    /// must be performed by the target contract itself via
    /// `env.deployer().update_current_contract_wasm()`.
    ///
    /// Only callable by an address holding the `Upgrader` role.
    pub fn execute_upgrade(
        env: Env,
        caller: Address,
        proposal_id: u64,
    ) -> Result<(), UpgradeError> {
        require_upgrader_role(&env, &caller)?;

        let mut proposal: UpgradeProposal = instance_get(&env, &(KEY_UPG_PROP, proposal_id))
            .ok_or(UpgradeError::ProposalNotFound)?;

        if proposal.executed {
            return Err(UpgradeError::AlreadyExecuted);
        }

        // Get the registry entry.
        let mut entry: RegistryEntry =
            instance_get(&env, &(KEY_REG_ENTRY, proposal.contract_id.clone()))
                .ok_or(UpgradeError::ContractNotRegistered)?;

        // Execute storage validation and pre-upgrade migration hook if present.
        if let Some(ref hook_addr) = entry.migration_hook {
            execute_validate_storage_hook(&env, hook_addr, &entry.contract_id)?;
            execute_pre_upgrade_hook(
                &env,
                hook_addr,
                &entry.contract_id,
                entry.current.version,
                proposal.new_version,
            )
            .map_err(|_| UpgradeError::MigrationHookFailed)?;
        }

        // Record old state for history.
        let old_version = entry.current.version;
        let old_wasm_hash = entry.current.wasm_hash.clone();

        // Update the registry entry with the new version.
        entry.current = VersionInfo {
            version: proposal.new_version,
            wasm_hash: proposal.new_wasm_hash.clone(),
            deployed_at: env.ledger().timestamp(),
            description: proposal.note.clone(),
        };
        instance_set(&env, &(KEY_REG_ENTRY, proposal.contract_id.clone()), &entry);

        // Execute post-upgrade migration hook if present.
        if let Some(ref hook_addr) = entry.migration_hook {
            execute_post_upgrade_hook(
                &env,
                hook_addr,
                &entry.contract_id,
                old_version,
                proposal.new_version,
            )
            .map_err(|_| UpgradeError::MigrationHookFailed)?;
        }

        // Mark proposal as executed.
        proposal.executed = true;
        proposal.executed_at = env.ledger().timestamp();
        instance_set(&env, &(KEY_UPG_PROP, proposal_id), &proposal);

        // Remove pending status.
        instance_remove(&env, &(KEY_PENDING, proposal.contract_id.clone()));

        // Record in upgrade history.
        let record = UpgradeRecord {
            contract_id: proposal.contract_id.clone(),
            old_version,
            new_version: proposal.new_version,
            old_wasm_hash,
            new_wasm_hash: proposal.new_wasm_hash.clone(),
            executor: caller.clone(),
            executed_at: env.ledger().timestamp(),
            note: proposal.note.clone(),
        };
        instance_set(
            &env,
            &(KEY_HISTORY, proposal.contract_id.clone(), proposal_id),
            &record,
        );

        events::emit_upgrade_executed(
            &env,
            proposal_id,
            &proposal.contract_id,
            old_version,
            proposal.new_version,
            &caller,
            env.ledger().timestamp(),
        );
        Ok(())
    }

    /// Returns a specific upgrade proposal by ID.
    pub fn get_proposal(env: Env, proposal_id: u64) -> Result<UpgradeProposal, UpgradeError> {
        instance_get(&env, &(KEY_UPG_PROP, proposal_id)).ok_or(UpgradeError::ProposalNotFound)
    }

    /// Returns the pending upgrade proposal ID for a contract, if any.
    pub fn get_pending_proposal(env: Env, contract_id: Address) -> Option<u64> {
        instance_get(&env, &(KEY_PENDING, contract_id))
    }

    /// Returns the upgrade status for a contract.
    pub fn get_upgrade_status(
        env: Env,
        contract_id: Address,
    ) -> Result<UpgradeStatus, UpgradeError> {
        // Verify contract is registered.
        if !instance_has(&env, &(KEY_REG_ENTRY, contract_id.clone())) {
            return Err(UpgradeError::ContractNotRegistered);
        }

        if let Some(proposal_id) = instance_get::<_, u64>(&env, &(KEY_PENDING, contract_id)) {
            Ok(UpgradeStatus::Pending(proposal_id))
        } else {
            Ok(UpgradeStatus::Current)
        }
    }

    /// Returns the upgrade history for a contract.
    ///
    /// Returns up to `max_results` records, starting from the most recent.
    pub fn get_upgrade_history(
        env: Env,
        contract_id: Address,
        max_results: u32,
    ) -> Vec<UpgradeRecord> {
        let entry: Option<RegistryEntry> =
            instance_get(&env, &(KEY_REG_ENTRY, contract_id.clone()));
        if entry.is_none() {
            return Vec::new(&env);
        }

        let mut records: Vec<UpgradeRecord> = Vec::new(&env);
        // Walk backwards from the proposal counter looking for records
        // belonging to this contract.
        let prop_count: u64 = instance_get(&env, &KEY_PROP_CNT).unwrap_or(0);
        let mut found = 0u32;
        let mut pid = prop_count;

        while pid >= 1 && found < max_results {
            let key = (KEY_HISTORY, contract_id.clone(), pid);
            if let Some(record) = instance_get::<_, UpgradeRecord>(&env, &key) {
                records.push_back(record);
                found += 1;
            }
            pid -= 1;
        }

        records
    }

    /// Cancel a pending upgrade proposal.
    ///
    /// Only callable by the original proposer or an admin.
    pub fn cancel_proposal(
        env: Env,
        caller: Address,
        proposal_id: u64,
    ) -> Result<(), UpgradeError> {
        let proposal: UpgradeProposal = instance_get(&env, &(KEY_UPG_PROP, proposal_id))
            .ok_or(UpgradeError::ProposalNotFound)?;

        if proposal.executed {
            return Err(UpgradeError::AlreadyExecuted);
        }

        // Only proposer or admin can cancel.
        let is_proposer = proposal.proposer == caller;
        let is_admin = auth::has_role(&env, &caller, Role::Admin);
        if !is_proposer && !is_admin {
            return Err(UpgradeError::NotUpgrader);
        }

        // Remove pending status.
        instance_remove(&env, &(KEY_PENDING, proposal.contract_id.clone()));

        // Remove the proposal entirely.
        instance_remove(&env, &(KEY_UPG_PROP, proposal_id));

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Upgrade execution helper (called by the target contract)
    // -----------------------------------------------------------------------

    /// Verify that an upgrade is authorized for a contract.
    ///
    /// This is called by the target contract's `upgrade` function to verify
    /// that the upgrade has been properly authorized through the registry.
    ///
    /// Returns the new WASM hash if the upgrade is authorized.
    pub fn verify_upgrade_authorization(
        env: Env,
        contract_id: Address,
        caller: Address,
        new_wasm_hash: BytesN<32>,
    ) -> Result<BytesN<32>, UpgradeError> {
        // Caller must hold the Upgrader role.
        if !auth::has_role(&env, &caller, Role::Upgrader) {
            return Err(UpgradeError::NotUpgrader);
        }

        // There must be a pending proposal matching this WASM hash.
        let pending_id: u64 = instance_get(&env, &(KEY_PENDING, contract_id.clone()))
            .ok_or(UpgradeError::ContractNotRegistered)?;

        let proposal: UpgradeProposal = instance_get(&env, &(KEY_UPG_PROP, pending_id))
            .ok_or(UpgradeError::ProposalNotFound)?;

        if proposal.new_wasm_hash != new_wasm_hash {
            return Err(UpgradeError::InvalidWasmHash);
        }

        Ok(proposal.new_wasm_hash)
    }

    // -----------------------------------------------------------------------
    // Admin helpers
    // -----------------------------------------------------------------------

    /// Returns `true` if the contract is registered.
    pub fn is_registered(env: Env, contract_id: Address) -> bool {
        instance_has(&env, &(KEY_REG_ENTRY, contract_id))
    }

    /// Returns `true` if the caller holds the Upgrader role.
    pub fn can_upgrade(env: Env, caller: Address) -> bool {
        auth::has_role(&env, &caller, Role::Upgrader)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Requires the caller to hold the `Admin` role.
fn require_admin_role(env: &Env, caller: &Address) -> ContractResult<()> {
    auth::require_role(env, caller, Role::Admin).map_err(|e| match e {
        Error::Unauthorized => UpgradeError::NotUpgrader,
        _ => UpgradeError::NotUpgrader,
    })
}

/// Requires the caller to hold the `Upgrader` role.
fn require_upgrader_role(env: &Env, caller: &Address) -> ContractResult<()> {
    auth::require_role(env, caller, Role::Upgrader).map_err(|e| match e {
        Error::Unauthorized => UpgradeError::NotUpgrader,
        _ => UpgradeError::NotUpgrader,
    })
}

/// Standard MigrationHook interface required for migration hook contracts.
pub trait MigrationHook {
    /// Validates storage layout compatibility and protocol invariants
    /// (e.g. total balances equal token reserves) in dry-run mode before an upgrade is applied.
    fn validate_storage(env: Env, target: Address) -> Result<(), Error>;

    /// Pre-upgrade logic executed before WASM bytecode replacement.
    fn pre_upgrade(env: Env, old_version: u32, new_version: u32) -> bool;

    /// Post-upgrade logic executed after WASM bytecode replacement.
    fn post_upgrade(env: Env, old_version: u32, new_version: u32);
}

/// Execute the storage validation dry-run hook.
///
/// Calls `validate_storage(target)` on the migration hook contract.
/// Verifies storage layout compatibility and critical protocol invariants.
fn execute_validate_storage_hook(
    env: &Env,
    hook_addr: &Address,
    target: &Address,
) -> Result<(), UpgradeError> {
    let args: soroban_sdk::Vec<Val> =
        soroban_sdk::Vec::from_array(env, [target.to_val()]);
    let result = env.try_invoke_contract::<(), Error>(
        hook_addr,
        &Symbol::new(env, "validate_storage"),
        args,
    );

    match result {
        Ok(Ok(())) => Ok(()),
        _ => Err(UpgradeError::StorageIncompatible),
    }
}

/// Execute the pre-upgrade migration hook.
///
/// Calls `pre_upgrade(old_version, new_version)` on the hook contract.
/// Returns Ok(true) if the hook approves the upgrade.
fn execute_pre_upgrade_hook(
    env: &Env,
    hook_addr: &Address,
    _contract_id: &Address,
    old_version: u32,
    new_version: u32,
) -> Result<(), UpgradeError> {
    // Cross-contract call to the migration hook.
    // The hook contract must implement: fn pre_upgrade(env, old_version: u32, new_version: u32) -> bool
    let args: soroban_sdk::Vec<Val> =
        soroban_sdk::Vec::from_array(env, [old_version.into_val(env), new_version.into_val(env)]);
    let result =
        env.try_invoke_contract::<bool, UpgradeError>(hook_addr, &symbol_short!("pre_upg"), args);

    match result {
        Ok(Ok(approved)) => {
            if approved {
                Ok(())
            } else {
                Err(UpgradeError::MigrationHookFailed)
            }
        }
        _ => Err(UpgradeError::MigrationHookFailed),
    }
}

/// Execute the post-upgrade migration hook.
///
/// Calls `post_upgrade(old_version, new_version)` on the hook contract.
fn execute_post_upgrade_hook(
    env: &Env,
    hook_addr: &Address,
    _contract_id: &Address,
    old_version: u32,
    new_version: u32,
) -> Result<(), UpgradeError> {
    let args: soroban_sdk::Vec<Val> =
        soroban_sdk::Vec::from_array(env, [old_version.into_val(env), new_version.into_val(env)]);
    let result =
        env.try_invoke_contract::<(), UpgradeError>(hook_addr, &symbol_short!("pst_upg"), args);

    match result {
        Ok(Ok(())) => Ok(()),
        _ => Err(UpgradeError::MigrationHookFailed),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use soroban_sdk::testutils::{Address as _, Events};
    use soroban_sdk::{Env, IntoVal};

    /// Creates a test environment with an initialized UpgradeabilityContract.
    /// Returns (env, client, admin).
    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, UpgradeabilityContract);
        let client = UpgradeabilityContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, contract_id, admin)
    }

    fn client_for<'a>(env: &'a Env, contract_id: &Address) -> UpgradeabilityContractClient<'a> {
        UpgradeabilityContractClient::new(env, contract_id)
    }

    /// Helper: create a fake WASM hash from a seed byte.
    fn fake_hash(env: &Env, seed: u8) -> BytesN<32> {
        let mut buf = [0u8; 32];
        buf[0] = seed;
        BytesN::from_array(env, &buf)
    }

    // -----------------------------------------------------------------------
    // initialize
    // -----------------------------------------------------------------------

    #[test]
    fn initialize_sets_admin_and_upgrader_role() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let _ = client;
        env.as_contract(&contract_id, || {
            assert!(auth::has_role(&env, &admin, Role::Admin));
            assert!(auth::has_role(&env, &admin, Role::Upgrader));
        });
    }

    // -----------------------------------------------------------------------
    // register_contract
    // -----------------------------------------------------------------------

    #[test]
    fn register_contract_succeeds() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm);

        assert!(client.is_registered(&contract_id));
        assert_eq!(client.get_version(&contract_id), 1);
        assert_eq!(client.get_wasm_hash(&contract_id), wasm);
    }

    #[test]
    fn register_duplicate_name_fails() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let c1 = Address::generate(&env);
        let c2 = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        client.register_contract(&admin, &c1, &symbol_short!("aid"), &1, &wasm);
        let result = client.try_register_contract(&admin, &c2, &symbol_short!("aid"), &1, &wasm);
        assert_eq!(result, Err(Ok(UpgradeError::ContractAlreadyRegistered)));
    }

    #[test]
    fn non_admin_cannot_register() {
        let (env, contract_id, _admin) = setup();
        let client = client_for(&env, &contract_id);
        let stranger = Address::generate(&env);
        let contract_id = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        let result =
            client.try_register_contract(&stranger, &contract_id, &symbol_short!("aid"), &1, &wasm);
        assert_eq!(result, Err(Ok(UpgradeError::NotUpgrader)));
    }

    #[test]
    fn register_zero_version_fails() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        let result =
            client.try_register_contract(&admin, &contract_id, &symbol_short!("aid"), &0, &wasm);
        assert_eq!(result, Err(Ok(UpgradeError::InvalidWasmHash)));
    }

    // -----------------------------------------------------------------------
    // get_registry_entry / get_registry_entry_by_name
    // -----------------------------------------------------------------------

    #[test]
    fn get_registry_entry_returns_correct_data() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm = fake_hash(&env, 42);

        client.register_contract(&admin, &contract_id, &symbol_short!("treasury"), &3, &wasm);

        let entry = client.get_registry_entry(&contract_id);
        assert_eq!(entry.name, symbol_short!("treasury"));
        assert_eq!(entry.current.version, 3);
        assert_eq!(entry.current.wasm_hash, wasm);
    }

    #[test]
    fn get_registry_entry_by_name_works() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm = fake_hash(&env, 7);

        client.register_contract(&admin, &contract_id, &symbol_short!("oracle"), &1, &wasm);

        let entry = client.get_registry_entry_by_name(&symbol_short!("oracle"));
        assert_eq!(entry.contract_id, contract_id);
    }

    #[test]
    fn unregistered_contract_returns_error() {
        let (env, contract_id, _admin) = setup();
        let client = client_for(&env, &contract_id);
        let unknown = Address::generate(&env);

        assert_eq!(
            client.try_get_registry_entry(&unknown),
            Err(Ok(UpgradeError::ContractNotRegistered))
        );
    }

    // -----------------------------------------------------------------------
    // Migration hooks
    // -----------------------------------------------------------------------

    #[test]
    fn set_migration_hook_succeeds() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let hook_addr = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm);
        client.set_migration_hook(&admin, &contract_id, &hook_addr);

        assert_eq!(client.get_migration_hook(&contract_id), Some(hook_addr));
    }

    #[test]
    fn non_admin_cannot_set_migration_hook() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let stranger = Address::generate(&env);
        let contract_id = Address::generate(&env);
        let hook_addr = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm);

        let result = client.try_set_migration_hook(&stranger, &contract_id, &hook_addr);
        assert_eq!(result, Err(Ok(UpgradeError::NotUpgrader)));
    }

    #[test]
    fn set_hook_on_unregistered_contract_fails() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let hook_addr = Address::generate(&env);

        let result = client.try_set_migration_hook(&admin, &contract_id, &hook_addr);
        assert_eq!(result, Err(Ok(UpgradeError::ContractNotRegistered)));
    }

    // -----------------------------------------------------------------------
    // propose_upgrade
    // -----------------------------------------------------------------------

    #[test]
    fn propose_upgrade_succeeds() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "upgrade to v2");
        let proposal_id = client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        assert_eq!(proposal_id, 1);
        let proposal = client.get_proposal(&proposal_id);
        assert_eq!(proposal.new_version, 2);
        assert_eq!(proposal.new_wasm_hash, wasm_v2);
        assert!(!proposal.executed);
    }

    #[test]
    fn propose_upgrade_same_version_fails() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm);

        let note = soroban_sdk::String::from_str(&env, "no-op");
        let result = client.try_propose_upgrade(
            &admin,
            &contract_id,
            &fake_hash(&env, 2),
            &1, // same version
            &note,
        );
        assert_eq!(result, Err(Ok(UpgradeError::NoChangeDetected)));
    }

    #[test]
    fn propose_upgrade_same_wasm_fails() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm);

        let note = soroban_sdk::String::from_str(&env, "same hash");
        let result = client.try_propose_upgrade(
            &admin,
            &contract_id,
            &wasm, // same hash
            &2,
            &note,
        );
        assert_eq!(result, Err(Ok(UpgradeError::NoChangeDetected)));
    }

    #[test]
    fn propose_duplicate_pending_fails() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "first");
        client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        let note2 = soroban_sdk::String::from_str(&env, "second");
        let result =
            client.try_propose_upgrade(&admin, &contract_id, &fake_hash(&env, 3), &3, &note2);
        assert_eq!(result, Err(Ok(UpgradeError::AlreadyPending)));
    }

    #[test]
    fn non_upgrader_cannot_propose() {
        let (env, registry_id, _admin) = setup();
        let client = client_for(&env, &registry_id);
        let stranger = Address::generate(&env);
        let target = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        // Register directly (bypass role check via internal state).
        let entry = RegistryEntry {
            contract_id: target.clone(),
            name: symbol_short!("aid"),
            current: VersionInfo {
                version: 1,
                wasm_hash: wasm.clone(),
                deployed_at: 0,
                description: soroban_sdk::String::from_str(&env, "init"),
            },
            migration_hook: None,
        };
        env.as_contract(&registry_id, || {
            instance_set(&env, &(KEY_REG_ENTRY, target.clone()), &entry);
            instance_set(&env, &(KEY_CONTRACT_BY_NAME, symbol_short!("aid")), &target);
        });

        let note = soroban_sdk::String::from_str(&env, "test");
        let result = client.try_propose_upgrade(&stranger, &target, &fake_hash(&env, 2), &2, &note);
        assert_eq!(result, Err(Ok(UpgradeError::NotUpgrader)));
    }

    // -----------------------------------------------------------------------
    // execute_upgrade
    // -----------------------------------------------------------------------

    #[test]
    fn execute_upgrade_updates_registry() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        let proposal_id = client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        client.execute_upgrade(&admin, &proposal_id);

        // Verify the registry was updated.
        let entry = client.get_registry_entry(&contract_id);
        assert_eq!(entry.current.version, 2);
        assert_eq!(entry.current.wasm_hash, wasm_v2);

        // Verify proposal is marked executed.
        let proposal = client.get_proposal(&proposal_id);
        assert!(proposal.executed);

        // No longer pending.
        assert_eq!(client.get_pending_proposal(&contract_id), None);
    }

    #[test]
    fn execute_upgrade_records_history() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        let proposal_id = client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        client.execute_upgrade(&admin, &proposal_id);

        let history = client.get_upgrade_history(&contract_id, &10);
        assert_eq!(history.len(), 1);
        let record = history.get(0).unwrap();
        assert_eq!(record.old_version, 1);
        assert_eq!(record.new_version, 2);
        assert_eq!(record.old_wasm_hash, wasm_v1);
        assert_eq!(record.new_wasm_hash, wasm_v2);
        assert_eq!(record.executor, admin);
    }

    #[test]
    fn execute_already_executed_fails() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        let proposal_id = client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        client.execute_upgrade(&admin, &proposal_id);

        let result = client.try_execute_upgrade(&admin, &proposal_id);
        assert_eq!(result, Err(Ok(UpgradeError::AlreadyExecuted)));
    }

    #[test]
    fn non_upgrader_cannot_execute() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let stranger = Address::generate(&env);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        let proposal_id = client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        let result = client.try_execute_upgrade(&stranger, &proposal_id);
        assert_eq!(result, Err(Ok(UpgradeError::NotUpgrader)));
    }

    #[test]
    fn execute_nonexistent_proposal_fails() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let result = client.try_execute_upgrade(&admin, &999);
        assert_eq!(result, Err(Ok(UpgradeError::ProposalNotFound)));
    }

    // -----------------------------------------------------------------------
    // cancel_proposal
    // -----------------------------------------------------------------------

    #[test]
    fn proposer_can_cancel() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        let proposal_id = client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        client.cancel_proposal(&admin, &proposal_id);

        // Proposal should be gone.
        let result = client.try_get_proposal(&proposal_id);
        assert_eq!(result, Err(Ok(UpgradeError::ProposalNotFound)));

        // No longer pending.
        assert_eq!(client.get_pending_proposal(&contract_id), None);
    }

    #[test]
    fn admin_can_cancel_others_proposals() {
        let (env, registry_id, admin) = setup();
        let client = client_for(&env, &registry_id);
        let upgrader = Address::generate(&env);
        let target = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        // Grant Upgrader role to upgrader.
        env.as_contract(&registry_id, || {
            persistent_set(
                &env,
                &auth::DataKey::Role(upgrader.clone(), Role::Upgrader),
                &true,
            );
        });

        client.register_contract(&admin, &target, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        let proposal_id = client.propose_upgrade(&upgrader, &target, &wasm_v2, &2, &note);

        // Admin cancels the upgrader's proposal.
        client.cancel_proposal(&admin, &proposal_id);

        let result = client.try_get_proposal(&proposal_id);
        assert_eq!(result, Err(Ok(UpgradeError::ProposalNotFound)));
    }

    #[test]
    fn cannot_cancel_executed_proposal() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        let proposal_id = client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        client.execute_upgrade(&admin, &proposal_id);

        let result = client.try_cancel_proposal(&admin, &proposal_id);
        assert_eq!(result, Err(Ok(UpgradeError::AlreadyExecuted)));
    }

    // -----------------------------------------------------------------------
    // verify_upgrade_authorization
    // -----------------------------------------------------------------------

    #[test]
    fn verify_upgrade_authorization_succeeds() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        let result = client.verify_upgrade_authorization(&contract_id, &admin, &wasm_v2);
        assert_eq!(result, wasm_v2);
    }

    #[test]
    fn verify_wrong_wasm_hash_fails() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);
        let wasm_wrong = fake_hash(&env, 99);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        let result = client.try_verify_upgrade_authorization(&contract_id, &admin, &wasm_wrong);
        assert_eq!(result, Err(Ok(UpgradeError::InvalidWasmHash)));
    }

    #[test]
    fn verify_unauthorized_caller_fails() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let stranger = Address::generate(&env);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        let result = client.try_verify_upgrade_authorization(&contract_id, &stranger, &wasm_v2);
        assert_eq!(result, Err(Ok(UpgradeError::NotUpgrader)));
    }

    // -----------------------------------------------------------------------
    // get_upgrade_status
    // -----------------------------------------------------------------------

    #[test]
    fn status_current_when_no_pending() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm);

        assert_eq!(
            client.get_upgrade_status(&contract_id),
            UpgradeStatus::Current
        );
    }

    #[test]
    fn status_pending_after_proposal() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        let proposal_id = client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        assert_eq!(
            client.get_upgrade_status(&contract_id),
            UpgradeStatus::Pending(proposal_id)
        );
    }

    // -----------------------------------------------------------------------
    // can_upgrade / is_registered
    // -----------------------------------------------------------------------

    #[test]
    fn can_upgrade_for_upgrader() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        assert!(client.can_upgrade(&admin));
    }

    #[test]
    fn can_upgrade_false_for_stranger() {
        let (env, contract_id, _admin) = setup();
        let client = client_for(&env, &contract_id);
        let stranger = Address::generate(&env);
        assert!(!client.can_upgrade(&stranger));
    }

    #[test]
    fn is_registered_true_for_registered() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm);
        assert!(client.is_registered(&contract_id));
    }

    #[test]
    fn is_registered_false_for_unknown() {
        let (env, contract_id, _admin) = setup();
        let unknown = Address::generate(&env);
        let client = UpgradeabilityContractClient::new(&env, &contract_id);
        assert!(!client.is_registered(&unknown));
    }

    // -----------------------------------------------------------------------
    // Events
    // -----------------------------------------------------------------------

    #[test]
    fn register_emits_event() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm);

        let all_events = env.events().all();
        let found = all_events
            .iter()
            .any(|e| e.1 == (symbol_short!("upgrade"), symbol_short!("upg_reg")).into_val(&env));
        assert!(found, "expected contract registered event");
    }

    #[test]
    fn propose_emits_event() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        let all_events = env.events().all();
        let found = all_events
            .iter()
            .any(|e| e.1 == (symbol_short!("upgrade"), symbol_short!("proposed")).into_val(&env));
        assert!(found, "expected upgrade proposed event");
    }

    #[test]
    fn execute_emits_event() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        let note = soroban_sdk::String::from_str(&env, "v2");
        let proposal_id = client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);

        client.execute_upgrade(&admin, &proposal_id);

        let all_events = env.events().all();
        let found = all_events
            .iter()
            .any(|e| e.1 == (symbol_short!("upgrade"), symbol_short!("executed")).into_val(&env));
        assert!(found, "expected upgrade executed event");
    }

    #[test]
    fn hook_set_emits_event() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let hook_addr = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm);
        client.set_migration_hook(&admin, &contract_id, &hook_addr);

        let all_events = env.events().all();
        let found = all_events
            .iter()
            .any(|e| e.1 == (symbol_short!("upgrade"), symbol_short!("hook_set")).into_val(&env));
        assert!(found, "expected migration hook set event");
    }

    // -----------------------------------------------------------------------
    // get_upgrade_history
    // -----------------------------------------------------------------------

    #[test]
    fn history_empty_for_no_upgrades() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm = fake_hash(&env, 1);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm);

        let history = client.get_upgrade_history(&contract_id, &10);
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn history_tracks_multiple_upgrades() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);
        let wasm_v3 = fake_hash(&env, 3);

        client.register_contract(&admin, &contract_id, &symbol_short!("aid"), &1, &wasm_v1);

        // Upgrade to v2
        let note = soroban_sdk::String::from_str(&env, "v2");
        let p1 = client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);
        client.execute_upgrade(&admin, &p1);

        // Upgrade to v3
        let note = soroban_sdk::String::from_str(&env, "v3");
        let p2 = client.propose_upgrade(&admin, &contract_id, &wasm_v3, &3, &note);
        client.execute_upgrade(&admin, &p2);

        let history = client.get_upgrade_history(&contract_id, &10);
        assert_eq!(history.len(), 2);

        // Most recent first.
        let r0 = history.get(0).unwrap();
        assert_eq!(r0.old_version, 2);
        assert_eq!(r0.new_version, 3);

        let r1 = history.get(1).unwrap();
        assert_eq!(r1.old_version, 1);
        assert_eq!(r1.new_version, 2);
    }

    // -----------------------------------------------------------------------
    // Full upgrade cycle
    // -----------------------------------------------------------------------

    #[test]
    fn full_upgrade_cycle_register_propose_execute() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let contract_id = Address::generate(&env);
        let wasm_v1 = fake_hash(&env, 1);
        let wasm_v2 = fake_hash(&env, 2);

        // 1. Register
        client.register_contract(
            &admin,
            &contract_id,
            &symbol_short!("treasury"),
            &1,
            &wasm_v1,
        );
        assert_eq!(client.get_version(&contract_id), 1);

        // 2. Propose
        let note = soroban_sdk::String::from_str(&env, "treasury v2");
        let pid = client.propose_upgrade(&admin, &contract_id, &wasm_v2, &2, &note);
        assert_eq!(client.get_pending_proposal(&contract_id), Some(pid));

        // 3. Execute
        client.execute_upgrade(&admin, &pid);

        // 4. Verify
        assert_eq!(client.get_version(&contract_id), 2);
        assert_eq!(client.get_wasm_hash(&contract_id), wasm_v2);
        assert_eq!(client.get_pending_proposal(&contract_id), None);

        // 5. History
        let history = client.get_upgrade_history(&contract_id, &10);
        assert_eq!(history.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Multi-contract registry
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_contracts_can_be_registered() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let aid = Address::generate(&env);
        let treasury = Address::generate(&env);
        let referral = Address::generate(&env);

        client.register_contract(&admin, &aid, &symbol_short!("aid"), &1, &fake_hash(&env, 1));
        client.register_contract(
            &admin,
            &treasury,
            &symbol_short!("treasury"),
            &1,
            &fake_hash(&env, 10),
        );
        client.register_contract(
            &admin,
            &referral,
            &symbol_short!("referral"),
            &1,
            &fake_hash(&env, 20),
        );

        assert_eq!(client.get_registered_count(), 3);
        assert!(client.is_registered(&aid));
        assert!(client.is_registered(&treasury));
        assert!(client.is_registered(&referral));

        // Each has independent state.
        assert_eq!(client.get_version(&aid), 1);
        assert_eq!(client.get_version(&treasury), 1);
        assert_eq!(client.get_version(&referral), 1);
    }

    // -----------------------------------------------------------------------
    // State migration dry-run and storage layout validation (Issue #97)
    // -----------------------------------------------------------------------

    #[contract]
    pub struct MockValidationHook;

    #[contractimpl]
    impl MockValidationHook {
        pub fn init(env: Env, compatible: bool) {
            env.storage().instance().set(&symbol_short!("compat"), &compatible);
        }

        pub fn validate_storage(env: Env, _target: Address) -> Result<(), Error> {
            let compatible: bool = env.storage().instance().get(&symbol_short!("compat")).unwrap_or(true);
            if compatible {
                Ok(())
            } else {
                Err(Error::InvalidArgument)
            }
        }

        pub fn pre_upg(_env: Env, _old_v: u32, _new_v: u32) -> bool {
            true
        }

        pub fn pst_upg(_env: Env, _old_v: u32, _new_v: u32) {}
    }

    #[test]
    fn test_propose_upgrade_validates_storage_success() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let target_contract = Address::generate(&env);

        let hook_addr = env.register_contract(None, MockValidationHook);
        let hook_client = MockValidationHookClient::new(&env, &hook_addr);
        hook_client.init(&true);

        client.register_contract(&admin, &target_contract, &symbol_short!("treasury"), &1, &fake_hash(&env, 1));
        client.set_migration_hook(&admin, &target_contract, &hook_addr);

        let note = soroban_sdk::String::from_str(&env, "valid layout upgrade");
        let pid = client.propose_upgrade(&admin, &target_contract, &fake_hash(&env, 2), &2, &note);
        assert_eq!(pid, 1);
    }

    #[test]
    fn test_propose_upgrade_rejects_incompatible_storage() {
        let (env, contract_id, admin) = setup();
        let client = client_for(&env, &contract_id);
        let target_contract = Address::generate(&env);

        let hook_addr = env.register_contract(None, MockValidationHook);
        let hook_client = MockValidationHookClient::new(&env, &hook_addr);
        hook_client.init(&false);

        client.register_contract(&admin, &target_contract, &symbol_short!("treasury"), &1, &fake_hash(&env, 1));
        client.set_migration_hook(&admin, &target_contract, &hook_addr);

        let note = soroban_sdk::String::from_str(&env, "breaking layout change");
        let result = client.try_propose_upgrade(&admin, &target_contract, &fake_hash(&env, 2), &2, &note);
        assert_eq!(result, Err(Ok(UpgradeError::StorageIncompatible)));
    }
}
