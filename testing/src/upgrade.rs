//! Upgrade simulation and testing utilities
//!
//! Provides test harnesses, mocks, and simulation tools for exercising
//! upgrade flows and state migrations across the Trellis protocol.

use soroban_sdk::{
    contract, contractimpl, symbol_short, testutils::Address as _, Address, BytesN, Env, Symbol,
    Vec,
};

use shared::auth::{self, Role};
use shared::storage::{instance_get, instance_set, persistent_set};

// ---------------------------------------------------------------------------
// Mock Migration Hook
// ---------------------------------------------------------------------------

/// A mock migration hook contract for testing pre/post upgrade callbacks.
///
/// Tracks whether `pre_upgrade` and `post_upgrade` were called, and allows
/// configuring whether `pre_upgrade` should approve or reject.
#[contract]
pub struct MockMigrationHook;

#[contractimpl]
impl MockMigrationHook {
    /// Initialize the hook with configurable approval behaviour.
    pub fn initialize(env: Env, admin: Address, approve: bool) {
        env.storage().instance().set(&symbol_short!("admin"), &admin);
        env.storage().instance().set(&symbol_short!("approve"), &approve);
        env.storage().instance().set(&symbol_short!("compat"), &true);
        env.storage().instance().set(&symbol_short!("pre_cnt"), &0u32);
        env.storage().instance().set(&symbol_short!("pst_cnt"), &0u32);
        env.storage().instance().set(&symbol_short!("val_cnt"), &0u32);
        env.storage().instance().set(&symbol_short!("last_old"), &0u32);
        env.storage().instance().set(&symbol_short!("last_new"), &0u32);
    }

    /// Set storage compatibility mode for validate_storage.
    pub fn set_storage_compatible(env: Env, compatible: bool) {
        env.storage().instance().set(&symbol_short!("compat"), &compatible);
    }

    /// Storage validation dry-run hook.
    ///
    /// Validates storage layout compatibility and protocol invariants (e.g. balances equal reserves).
    pub fn validate_storage(env: Env, target: Address) -> Result<(), shared::errors::Error> {
        let compatible: bool = env.storage().instance().get(&symbol_short!("compat")).unwrap_or(true);
        if !compatible {
            return Err(shared::errors::Error::InvalidArgument);
        }
        let mut count: u32 = env.storage().instance().get(&symbol_short!("val_cnt")).unwrap_or(0);
        count += 1;
        env.storage().instance().set(&symbol_short!("val_cnt"), &count);
        env.storage().instance().set(&symbol_short!("target"), &target);
        Ok(())
    }

    /// Pre-upgrade hook. Returns `true` if the upgrade is approved.
    pub fn pre_upg(env: Env, old_version: u32, new_version: u32) -> bool {
        let approve: bool = env.storage().instance().get(&symbol_short!("approve")).unwrap();
        let mut count: u32 = env.storage().instance().get(&symbol_short!("pre_cnt")).unwrap_or(0);
        count += 1;
        env.storage().instance().set(&symbol_short!("pre_cnt"), &count);
        env.storage().instance().set(&symbol_short!("last_old"), &old_version);
        env.storage().instance().set(&symbol_short!("last_new"), &new_version);
        approve
    }

    /// Post-upgrade hook. Always succeeds.
    pub fn pst_upg(env: Env, old_version: u32, new_version: u32) {
        let mut count: u32 = env.storage().instance().get(&symbol_short!("pst_cnt")).unwrap_or(0);
        count += 1;
        env.storage().instance().set(&symbol_short!("pst_cnt"), &count);
        env.storage().instance().set(&symbol_short!("last_old"), &old_version);
        env.storage().instance().set(&symbol_short!("last_new"), &new_version);
    }

    /// Returns how many times `validate_storage` was called.
    pub fn val_call_count(env: Env) -> u32 {
        env.storage().instance().get(&symbol_short!("val_cnt")).unwrap_or(0)
    }

    /// Returns how many times `pre_upg` was called.
    pub fn pre_call_count(env: Env) -> u32 {
        env.storage().instance().get(&symbol_short!("pre_cnt")).unwrap_or(0)
    }

    /// Returns how many times `pst_upg` was called.
    pub fn post_call_count(env: Env) -> u32 {
        env.storage().instance().get(&symbol_short!("pst_cnt")).unwrap_or(0)
    }

    /// Returns the last (old_version, new_version) seen by either hook.
    pub fn last_versions(env: Env) -> (u32, u32) {
        let old: u32 = env.storage().instance().get(&symbol_short!("last_old")).unwrap_or(0);
        let new: u32 = env.storage().instance().get(&symbol_short!("last_new")).unwrap_or(0);
        (old, new)
    }
}

/// Create and initialize a mock migration hook.
pub fn create_mock_migration_hook(
    env: &Env,
    admin: &Address,
    approve: bool,
) -> (Address, MockMigrationHookClient) {
    let addr = env.register(MockMigrationHook, ());
    let client = MockMigrationHookClient::new(env, &addr);
    client.initialize(admin, &approve);
    (addr, MockMigrationHookClient::new(env, &addr))
}

// ---------------------------------------------------------------------------
// Mock Upgradeable Contract
// ---------------------------------------------------------------------------

/// A simple contract that stores a version number and supports simulated
/// upgrades for testing the UpgradeabilityContract's flow end-to-end.
#[contract]
pub struct MockUpgradeableContract;

#[contractimpl]
impl MockUpgradeableContract {
    /// Initialize with a version number and admin.
    pub fn initialize(env: Env, admin: Address, version: u32) {
        env.storage().instance().set(&b"admin", &admin);
        env.storage().instance().set(&b"version", &version);
    }

    /// Simulate an upgrade by updating the version number.
    ///
    /// In a real contract, this would call
    /// `env.deployer().update_current_contract_wasm(new_wasm_hash)`.
    pub fn simulate_upgrade(env: Env, caller: Address, new_version: u32) {
        // Verify the caller holds the Upgrader role (same as real upgrade).
        if !auth::has_role(&env, &caller, Role::Upgrader) {
            panic!("not authorized");
        }

        let old_version: u32 = env.storage().instance().get(&b"version").unwrap_or(0);
        env.storage().instance().set(&b"version", &new_version);

        // Emit a simulated upgrade event.
        env.events().publish(
            (symbol_short!("sim_upg"),),
            (old_version, new_version, caller),
        );
    }

    /// Returns the current version.
    pub fn get_version(env: Env) -> u32 {
        env.storage().instance().get(&b"version").unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Upgrade Test Harness
// ---------------------------------------------------------------------------

/// A test harness for end-to-end upgrade flow testing.
pub struct UpgradeTestHarness {
    pub env: Env,
    pub admin: Address,
    pub upgrader: Address,
    pub upgradeability_addr: Address,
    pub migration_hook_addr: Option<Address>,
}

impl UpgradeTestHarness {
    /// Create a new harness with the upgradeability registry initialized.
    pub fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let upgrader = Address::generate(&env);

        // Register the upgradeability contract.
        let upgradeability_addr =
            env.register_contract(None, upgradeability::UpgradeabilityContract);

        // Initialize the upgradeability contract.
        env.invoke_contract(
            &upgradeability_addr,
            &Symbol::new(&env, "initialize"),
            (admin.clone(),),
        );

        // Grant Upgrader role to the upgrader address.
        persistent_set(
            &env,
            &auth::DataKey::Role(upgrader.clone(), Role::Upgrader),
            &true,
        );

        Self {
            env,
            admin,
            upgrader,
            upgradeability_addr,
            migration_hook_addr: None,
        }
    }

    /// Register a mock migration hook for testing.
    pub fn setup_migration_hook(&mut self, approve: bool) -> Address {
        let (addr, _) = create_mock_migration_hook(&self.env, &self.admin, approve);
        self.migration_hook_addr = Some(addr.clone());
        addr
    }

    /// Register a contract in the upgrade registry.
    pub fn register_contract(
        &self,
        contract_id: &Address,
        name: Symbol,
        version: u32,
        wasm_hash: BytesN<32>,
    ) {
        self.env.invoke_contract(
            &self.upgradeability_addr,
            &Symbol::new(&self.env, "register_contract"),
            (
                self.admin.clone(),
                contract_id.clone(),
                name,
                version,
                wasm_hash,
            ),
        );
    }

    /// Set a migration hook for a registered contract.
    pub fn set_migration_hook(&self, contract_id: &Address, hook_addr: &Address) {
        self.env.invoke_contract(
            &self.upgradeability_addr,
            &Symbol::new(&self.env, "set_migration_hook"),
            (self.admin.clone(), contract_id.clone(), hook_addr.clone()),
        );
    }

    /// Propose an upgrade.
    pub fn propose_upgrade(
        &self,
        contract_id: &Address,
        new_wasm_hash: BytesN<32>,
        new_version: u32,
        note: &str,
    ) -> u64 {
        self.env.invoke_contract(
            &self.upgradeability_addr,
            &Symbol::new(&self.env, "propose_upgrade"),
            (
                self.upgrader.clone(),
                contract_id.clone(),
                new_wasm_hash,
                new_version,
                soroban_sdk::String::from_str(&self.env, note),
            ),
        )
    }

    /// Execute an upgrade.
    pub fn execute_upgrade(&self, proposal_id: u64) {
        self.env.invoke_contract(
            &self.upgradeability_addr,
            &Symbol::new(&self.env, "execute_upgrade"),
            (self.upgrader.clone(), proposal_id),
        );
    }
}

/// Create a fake WASM hash from a seed byte for testing.
pub fn fake_wasm_hash(seed: u8) -> BytesN<32> {
    let mut buf = [0u8; 32];
    buf[0] = seed;
    BytesN::from_array(&Env::default(), &buf)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn mock_migration_hook_approve() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);

        let (addr, client) = create_mock_migration_hook(&env, &admin, true);
        assert!(client.pre_upg(&1, &2));
        assert_eq!(client.pre_call_count(), 1);
        assert_eq!(client.last_versions(), (1, 2));

        client.pst_upg(&1, &2);
        assert_eq!(client.post_call_count(), 1);
    }

    #[test]
    fn mock_migration_hook_reject() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);

        let (_, client) = create_mock_migration_hook(&env, &admin, false);
        assert!(!client.pre_upg(&1, &2));
    }

    #[test]
    fn mock_upgradeable_contract_version_tracking() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);

        let addr = env.register(MockUpgradeableContract, ());
        let client = MockUpgradeableContractClient::new(&env, &addr);
        client.initialize(&admin, &1);
        assert_eq!(client.get_version(), 1);

        // Grant Upgrader role to admin.
        persistent_set(
            &env,
            &auth::DataKey::Role(admin.clone(), Role::Upgrader),
            &true,
        );

        client.simulate_upgrade(&admin, &2);
        assert_eq!(client.get_version(), 2);
    }

    #[test]
    fn harness_end_to_end_upgrade_flow() {
        let mut harness = UpgradeTestHarness::new();

        // Register a mock contract.
        let contract_id = Address::generate(&harness.env);
        harness.register_contract(&contract_id, symbol_short!("aid"), 1, fake_wasm_hash(1));

        // Propose upgrade.
        let proposal_id =
            harness.propose_upgrade(&contract_id, fake_wasm_hash(2), 2, "upgrade to v2");

        // Execute upgrade.
        harness.execute_upgrade(proposal_id);

        // Verify via registry call.
        let version: u32 = harness.env.invoke_contract(
            &harness.upgradeability_addr,
            &Symbol::new(&harness.env, "get_version"),
            (contract_id.clone(),),
        );
        assert_eq!(version, 2);
    }

    #[test]
    fn harness_with_migration_hook() {
        let mut harness = UpgradeTestHarness::new();

        // Setup migration hook that approves.
        let hook_addr = harness.setup_migration_hook(true);

        // Register a contract.
        let contract_id = Address::generate(&harness.env);
        harness.register_contract(
            &contract_id,
            symbol_short!("treasury"),
            1,
            fake_wasm_hash(1),
        );

        // Set migration hook.
        harness.set_migration_hook(&contract_id, &hook_addr);

        // Propose and execute.
        let pid = harness.propose_upgrade(&contract_id, fake_wasm_hash(2), 2, "v2 with migration");
        harness.execute_upgrade(pid);

        // Verify hook was called.
        let pre_count: u32 = harness.env.invoke_contract(
            &hook_addr,
            &Symbol::new(&harness.env, "pre_call_count"),
            (),
        );
        let post_count: u32 = harness.env.invoke_contract(
            &hook_addr,
            &Symbol::new(&harness.env, "post_call_count"),
            (),
        );
        assert_eq!(pre_count, 1);
        assert_eq!(post_count, 1);
    }

    #[test]
    fn harness_rejects_incompatible_storage_layout() {
        let mut harness = UpgradeTestHarness::new();

        // Setup migration hook that rejects storage compatibility (simulating breaking schema).
        let hook_addr = harness.setup_migration_hook(true);
        let hook_client = MockMigrationHookClient::new(&harness.env, &hook_addr);
        hook_client.set_storage_compatible(&false);

        // Register a contract.
        let contract_id = Address::generate(&harness.env);
        harness.register_contract(
            &contract_id,
            symbol_short!("aid"),
            1,
            fake_wasm_hash(1),
        );

        // Set migration hook.
        harness.set_migration_hook(&contract_id, &hook_addr);

        // Propose upgrade — must fail dry-run validation because storage is incompatible!
        let note = soroban_sdk::String::from_str(&harness.env, "v2 with breaking schema");
        let result = harness.env.try_invoke_contract::<u64, upgradeability::UpgradeError>(
            &harness.upgradeability_addr,
            &Symbol::new(&harness.env, "propose_upgrade"),
            soroban_sdk::Vec::from_array(
                &harness.env,
                [
                    harness.upgrader.clone().into_val(&harness.env),
                    contract_id.clone().into_val(&harness.env),
                    fake_wasm_hash(2).into_val(&harness.env),
                    2u32.into_val(&harness.env),
                    note.into_val(&harness.env),
                ],
            ),
        );

        assert_eq!(result, Err(Ok(upgradeability::UpgradeError::StorageIncompatible)));
    }
}
