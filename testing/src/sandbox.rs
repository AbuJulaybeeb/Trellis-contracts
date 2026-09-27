//! Integration sandbox mode for safe contributor testing (Issue #57).
//!
//! Sandbox mode lets a contributor exercise the primary Trellis workflow
//! locally against *fake* external services — oracle, token, and RPC — without
//! production credentials, real wallets, or irreversible records.
//!
//! ## Design
//!
//! - [`SandboxConfig`] is validated up front: mainnet is rejected outright and
//!   local runs may only target loopback endpoints.
//! - Every adapter is deterministic. Nothing reads wall-clock time or OS
//!   entropy: the only variability is [`DeterministicRng`], seeded from
//!   [`SandboxConfig::deterministic_seed`], so two runs of the same fixture
//!   produce identical results.
//! - Irreversible side effects cannot happen by construction —
//!   [`FakeRpcAdapter::submit`] appends to an in-memory log instead of
//!   broadcasting, and [`FakeTokenAdapter`] balances live in the test process.
//! - [`standard_fixtures`] ships success and failure scenarios and
//!   [`run_fixture`] executes one, reporting whether the expectation held.
//! - [`Sandbox::assert_credential_free`] and [`Sandbox::assert_endpoint_allowed`]
//!   make the "no production credentials / no production endpoints" rule
//!   testable rather than a convention.
//!
//! See `docs/SANDBOX.md` for local setup, limitations, and the validation
//! command.

use soroban_sdk::{
    contracttype, symbol_short, testutils::Address as _, Address, Env, Map, Symbol, Vec,
};

use shared::config::{validate_rpc_url, validate_secret_key, Environment};
use shared::Error;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Ledgers per day at the ~5 s/ledger cadence the repository targets.
pub const LEDGERS_PER_DAY: u32 = 17_280;

/// Seed used when the caller does not choose one.
pub const DEFAULT_SEED: u64 = 0x5EED_0000_0000_0001;

/// Upper bound on simulated adapter latency, so fixtures stay fast.
pub const MAX_SIMULATED_LATENCY_LEDGERS: u32 = 100;

/// Flat fee applied by the sandboxed settlement workflow (50 bps).
pub const SANDBOX_FEE_BPS: i128 = 50;

/// Endpoint prefixes a [`SandboxNetwork::Local`] run is allowed to talk to.
pub const LOOPBACK_PREFIXES: [&str; 3] = ["http://localhost", "http://127.0.0.1", "http://[::1]"];

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Network a sandbox run targets.
///
/// [`SandboxNetwork::Mainnet`] exists only so the guard can reject it — the
/// sandbox never signs, submits, or settles against mainnet.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxNetwork {
    /// Fully local: ephemeral accounts, in-memory ledger, loopback only.
    Local,
    /// Stellar testnet: test-only assets, no production keys.
    Testnet,
    /// Rejected by validation.
    Mainnet,
}

impl SandboxNetwork {
    pub fn is_mainnet(self) -> bool {
        self == SandboxNetwork::Mainnet
    }
}

/// Sandbox behaviour, fixed before any adapter is constructed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxConfig {
    pub network: SandboxNetwork,
    /// Seed for all pseudo-random sandbox decisions.
    pub deterministic_seed: u64,
    /// Ledger the fake RPC starts from.
    pub start_ledger: u32,
    /// Unix timestamp the fake clock starts from.
    pub start_timestamp: u64,
    /// Ledgers added to every adapter response (simulated latency).
    pub simulated_latency_ledgers: u32,
    /// Reject any run that presents key material or a non-loopback endpoint.
    pub forbid_production_credentials: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            network: SandboxNetwork::Local,
            deterministic_seed: DEFAULT_SEED,
            start_ledger: 1,
            start_timestamp: 1_640_995_200, // 2022-01-01T00:00:00Z, matches test helpers
            simulated_latency_ledgers: 1,
            forbid_production_credentials: true,
        }
    }
}

impl SandboxConfig {
    /// Local sandbox with a fixed seed.
    pub fn new(seed: u64) -> Self {
        Self {
            deterministic_seed: seed,
            ..Self::default()
        }
    }

    /// Validate the sandbox itself. Mainnet and zeroed values are refused.
    pub fn validate(&self) -> Result<(), Error> {
        if self.network.is_mainnet() {
            return Err(Error::ConfigInvalid);
        }
        if self.deterministic_seed == 0 || self.start_ledger == 0 {
            return Err(Error::ConfigInvalid);
        }
        if self.simulated_latency_ledgers > MAX_SIMULATED_LATENCY_LEDGERS {
            return Err(Error::ConfigInvalid);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Deterministic randomness
// ---------------------------------------------------------------------------

/// Small linear-congruential generator: no OS entropy, fully reproducible.
#[derive(Clone, Debug)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    const MULTIPLIER: u64 = 6_364_136_223_846_793_005;
    const INCREMENT: u64 = 1_442_695_040_888_963_407;

    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { DEFAULT_SEED } else { seed },
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::INCREMENT);
        self.state
    }

    /// Uniform value in `[lo, hi)` (returns `lo` when the range is empty).
    pub fn next_i128(&mut self, lo: i128, hi: i128) -> i128 {
        if hi <= lo {
            return lo;
        }
        let span = (hi - lo) as u128;
        let value = (self.next_u64() as u128) % span;
        lo + value as i128
    }

    pub fn next_bool(&mut self) -> bool {
        self.next_u64() % 2 == 0
    }
}

// ---------------------------------------------------------------------------
// Adapters
// ---------------------------------------------------------------------------

/// A recorded adapter call — the sandbox's own audit trail for assertions.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterCall {
    pub adapter: Symbol,
    pub operation: Symbol,
    /// `false` when the call was refused by a guard before reaching an adapter.
    pub allowed: bool,
}

/// Deterministic external price feed.
///
/// Unknown assets and assets marked failing return [`Error::NotFound`], which
/// models an upstream outage without inventing a new error code.
#[derive(Clone, Debug)]
pub struct FakeOracleAdapter {
    prices: Map<Symbol, i128>,
    failing: Map<Symbol, bool>,
    stale: Map<Symbol, bool>,
    latency_ledgers: u32,
    quotes_served: u32,
}

/// One price reading, tagged with the simulated staleness flag.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceQuote {
    pub asset: Symbol,
    pub price: i128,
    pub timestamp: u64,
    pub stale: bool,
}

impl FakeOracleAdapter {
    pub fn new(env: &Env, latency_ledgers: u32) -> Self {
        Self {
            prices: Map::new(env),
            failing: Map::new(env),
            stale: Map::new(env),
            latency_ledgers,
            quotes_served: 0,
        }
    }

    /// Register a price. Non-positive prices are rejected.
    pub fn set_price(&mut self, asset: &Symbol, price: i128) -> Result<(), Error> {
        if price <= 0 {
            return Err(Error::InvalidAmount);
        }
        self.prices.set(asset.clone(), price);
        Ok(())
    }

    /// Make this asset fail (simulated upstream outage).
    pub fn fail_for(&mut self, asset: &Symbol) {
        self.failing.set(asset.clone(), true);
    }

    /// Mark this asset's price as stale.
    pub fn mark_stale(&mut self, asset: &Symbol) {
        self.stale.set(asset.clone(), true);
    }

    pub fn quote(&mut self, env: &Env, asset: &Symbol) -> Result<PriceQuote, Error> {
        if self.failing.get(asset.clone()).unwrap_or(false) {
            return Err(Error::NotFound);
        }
        let price = self.prices.get(asset.clone()).ok_or(Error::NotFound)?;
        self.quotes_served = self.quotes_served.saturating_add(1);
        Ok(PriceQuote {
            asset: asset.clone(),
            price,
            timestamp: env.ledger().timestamp(),
            stale: self.stale.get(asset.clone()).unwrap_or(false),
        })
    }

    pub fn latency_ledgers(&self) -> u32 {
        self.latency_ledgers
    }

    pub fn quotes_served(&self) -> u32 {
        self.quotes_served
    }
}

/// Deterministic in-process token: no real wallets, no ledger writes.
#[derive(Clone, Debug)]
pub struct FakeTokenAdapter {
    balances: Map<Address, i128>,
    supply: i128,
    latency_ledgers: u32,
    transfers: u32,
}

impl FakeTokenAdapter {
    pub fn new(env: &Env, latency_ledgers: u32) -> Self {
        Self {
            balances: Map::new(env),
            supply: 0,
            latency_ledgers,
            transfers: 0,
        }
    }

    /// Sandbox faucet. Production minting paths are never reachable here.
    pub fn mint(&mut self, to: &Address, amount: i128) -> Result<(), Error> {
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let current = self.balances.get(to.clone()).unwrap_or(0);
        let next = current.checked_add(amount).ok_or(Error::Overflow)?;
        self.balances.set(to.clone(), next);
        self.supply = self.supply.checked_add(amount).ok_or(Error::Overflow)?;
        Ok(())
    }

    pub fn balance_of(&self, who: &Address) -> i128 {
        self.balances.get(who.clone()).unwrap_or(0)
    }

    pub fn transfer(&mut self, from: &Address, to: &Address, amount: i128) -> Result<(), Error> {
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let from_balance = self.balance_of(from);
        if from_balance < amount {
            return Err(Error::InsufficientBalance);
        }
        let to_balance = self.balance_of(to);
        self.balances
            .set(from.clone(), from_balance.saturating_sub(amount));
        self.balances
            .set(to.clone(), to_balance.saturating_add(amount));
        self.transfers = self.transfers.saturating_add(1);
        Ok(())
    }

    pub fn total_supply(&self) -> i128 {
        self.supply
    }

    pub fn transfers(&self) -> u32 {
        self.transfers
    }

    pub fn latency_ledgers(&self) -> u32 {
        self.latency_ledgers
    }
}

/// Fake RPC: keeps submitted operations in memory, never broadcasts.
#[derive(Clone, Debug)]
pub struct FakeRpcAdapter {
    ledger: u32,
    timestamp: u64,
    latency_ledgers: u32,
    fail_next: bool,
    submitted: Vec<Symbol>,
}

impl FakeRpcAdapter {
    pub fn new(env: &Env, start_ledger: u32, start_timestamp: u64, latency_ledgers: u32) -> Self {
        Self {
            ledger: start_ledger,
            timestamp: start_timestamp,
            latency_ledgers,
            fail_next: false,
            submitted: Vec::new(env),
        }
    }

    pub fn ledger(&self) -> u32 {
        self.ledger
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Advance the simulated clock.
    pub fn advance(&mut self, ledgers: u32) {
        self.ledger = self.ledger.saturating_add(ledgers);
        self.timestamp = self
            .timestamp
            .saturating_add((ledgers as u64).saturating_mul(5));
    }

    /// Make the next submission fail (simulated inclusion timeout).
    pub fn set_fail_next(&mut self, fail: bool) {
        self.fail_next = fail;
    }

    /// Submit an operation. Returns the ledger it would land in.
    pub fn submit(&mut self, op: &Symbol) -> Result<u32, Error> {
        if self.fail_next {
            self.fail_next = false;
            return Err(Error::Expired);
        }
        self.submitted.push_back(op.clone());
        self.advance(self.latency_ledgers);
        Ok(self.ledger)
    }

    pub fn submitted_count(&self) -> u32 {
        self.submitted.len()
    }
}

// ---------------------------------------------------------------------------
// Sandbox
// ---------------------------------------------------------------------------

/// Report produced by [`Sandbox::run_primary_workflow`].
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowReport {
    pub steps: u32,
    pub amount: i128,
    pub fee: i128,
    pub oracle_price: i128,
    pub payer_balance: i128,
    pub payee_balance: i128,
    pub settled_ledger: u32,
    /// `true` when the run was guaranteed credential-free.
    pub credential_free: bool,
}

/// A sandboxed environment: validated config plus fake external services.
pub struct Sandbox {
    config: SandboxConfig,
    rng: DeterministicRng,
    oracle: FakeOracleAdapter,
    token: FakeTokenAdapter,
    rpc: FakeRpcAdapter,
    calls: Vec<AdapterCall>,
}

impl Sandbox {
    /// Build a sandbox. Fails fast on an invalid configuration.
    pub fn new(env: &Env, config: SandboxConfig) -> Result<Self, Error> {
        config.validate()?;
        let rng = DeterministicRng::new(config.deterministic_seed);
        Ok(Self {
            oracle: FakeOracleAdapter::new(env, config.simulated_latency_ledgers),
            token: FakeTokenAdapter::new(env, config.simulated_latency_ledgers),
            rpc: FakeRpcAdapter::new(
                env,
                config.start_ledger,
                config.start_timestamp,
                config.simulated_latency_ledgers,
            ),
            calls: Vec::new(env),
            rng,
            config,
        })
    }

    /// Build a sandbox and enforce the credential / endpoint guardrails.
    pub fn new_checked(
        env: &Env,
        config: SandboxConfig,
        secret: Option<&str>,
        endpoint: Option<&str>,
    ) -> Result<Self, Error> {
        let sandbox = Self::new(env, config)?;
        sandbox.assert_credential_free(secret)?;
        if let Some(url) = endpoint {
            sandbox.assert_endpoint_allowed(url)?;
        }
        Ok(sandbox)
    }

    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }

    pub fn oracle(&self) -> &FakeOracleAdapter {
        &self.oracle
    }

    pub fn oracle_mut(&mut self) -> &mut FakeOracleAdapter {
        &mut self.oracle
    }

    pub fn token(&self) -> &FakeTokenAdapter {
        &self.token
    }

    pub fn token_mut(&mut self) -> &mut FakeTokenAdapter {
        &mut self.token
    }

    pub fn rpc(&self) -> &FakeRpcAdapter {
        &self.rpc
    }

    pub fn rpc_mut(&mut self) -> &mut FakeRpcAdapter {
        &mut self.rpc
    }

    pub fn rng_mut(&mut self) -> &mut DeterministicRng {
        &mut self.rng
    }

    pub fn calls(&self) -> &Vec<AdapterCall> {
        &self.calls
    }

    /// Refuse key material. Sandbox tests must not need real credentials.
    ///
    /// With [`SandboxConfig::forbid_production_credentials`] set, *any*
    /// non-empty secret is rejected with [`Error::UnsafeSecret`]; the shape
    /// check from `shared::config` is applied when the guard is disabled.
    pub fn assert_credential_free(&self, secret: Option<&str>) -> Result<(), Error> {
        let presented = secret.map(str::trim).filter(|s| !s.is_empty());
        match presented {
            None => Ok(()),
            Some(_) if self.config.forbid_production_credentials => Err(Error::UnsafeSecret),
            Some(value) => validate_secret_key(Some(value), Environment::Local),
        }
    }

    /// Refuse production endpoints: local runs stay on loopback, testnet runs
    /// must be well-formed URLs, and mainnet is never allowed.
    pub fn assert_endpoint_allowed(&self, rpc_url: &str) -> Result<(), Error> {
        let url = rpc_url.trim();
        if url.is_empty() || url.contains(' ') {
            return Err(Error::ConfigInvalid);
        }
        match self.config.network {
            SandboxNetwork::Mainnet => Err(Error::ConfigInvalid),
            SandboxNetwork::Local => {
                if LOOPBACK_PREFIXES
                    .iter()
                    .any(|prefix| url.starts_with(*prefix))
                {
                    Ok(())
                } else {
                    Err(Error::ConfigInvalid)
                }
            }
            SandboxNetwork::Testnet => {
                if url.starts_with("http://") || url.starts_with("https://") {
                    validate_rpc_url(Some(url))
                } else {
                    Err(Error::ConfigInvalid)
                }
            }
        }
    }

    /// Run the primary workflow — fund, quote, settle — entirely in-process.
    ///
    /// The payer must be funded beforehand via [`FakeTokenAdapter::mint`]; the
    /// workflow never creates value on its own.
    pub fn run_primary_workflow(
        &mut self,
        env: &Env,
        payer: &Address,
        payee: &Address,
        oracle_asset: &Symbol,
        amount: i128,
    ) -> Result<WorkflowReport, Error> {
        self.record_call(&symbol_short!("start"), &symbol_short!("wf"), true);
        if amount <= 0 {
            self.record_call(&symbol_short!("token"), &symbol_short!("amt"), false);
            return Err(Error::InvalidAmount);
        }

        let quote = match self.oracle.quote(env, oracle_asset) {
            Ok(quote) => {
                self.record_call(&symbol_short!("oracle"), &symbol_short!("quote"), true);
                quote
            }
            Err(err) => {
                self.record_call(&symbol_short!("oracle"), &symbol_short!("quote"), false);
                return Err(err);
            }
        };

        let fee = estimate_fee(amount)?;
        let net = amount.saturating_sub(fee);
        if let Err(err) = self.token.transfer(payer, payee, net) {
            self.record_call(&symbol_short!("token"), &symbol_short!("xfer"), false);
            return Err(err);
        }
        self.record_call(&symbol_short!("token"), &symbol_short!("xfer"), true);

        let settled = match self.rpc.submit(&symbol_short!("settle")) {
            Ok(ledger) => {
                self.record_call(&symbol_short!("rpc"), &symbol_short!("settle"), true);
                ledger
            }
            Err(err) => {
                self.record_call(&symbol_short!("rpc"), &symbol_short!("settle"), false);
                return Err(err);
            }
        };

        // Seeded jitter makes the confirmation ledger vary per run without
        // making it unpredictable.
        let jitter = self
            .rng
            .next_i128(0, self.config.simulated_latency_ledgers as i128 + 1);
        Ok(WorkflowReport {
            steps: 4,
            amount,
            fee,
            oracle_price: quote.price,
            payer_balance: self.token.balance_of(payer),
            payee_balance: self.token.balance_of(payee),
            settled_ledger: settled.saturating_add(jitter as u32),
            credential_free: self.config.forbid_production_credentials,
        })
    }

    fn record_call(&mut self, adapter: &Symbol, operation: &Symbol, allowed: bool) {
        self.calls.push_back(AdapterCall {
            adapter: adapter.clone(),
            operation: operation.clone(),
            allowed,
        });
    }
}

/// Sandbox fee in basis points, saturating into [`Error::Overflow`] on absurd
/// amounts rather than wrapping.
pub fn estimate_fee(amount: i128) -> Result<i128, Error> {
    amount
        .checked_mul(SANDBOX_FEE_BPS)
        .map(|scaled| scaled / 10_000)
        .ok_or(Error::Overflow)
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// What a fixture expects to happen.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScenarioExpectation {
    Success,
    Failure,
}

/// A declared scenario: the inputs a sandbox run starts from.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioFixture {
    pub name: Symbol,
    pub expectation: ScenarioExpectation,
    /// Price the fake oracle returns for the traded asset.
    pub oracle_price: i128,
    /// When `true` the oracle fails for the traded asset.
    pub oracle_failure: bool,
    /// Amount routed through the workflow.
    pub amount: i128,
    /// Payer funding; `0` means "fund exactly `amount`".
    pub funding: i128,
    /// When `true` the fake RPC rejects settlement.
    pub rpc_failure: bool,
}

impl ScenarioFixture {
    /// Effective payer funding for this fixture.
    pub fn funding_amount(&self) -> i128 {
        if self.funding > 0 {
            self.funding
        } else {
            self.amount
        }
    }

    /// Whether the fixture declares a failure.
    pub fn expects_failure(&self) -> bool {
        self.expectation == ScenarioExpectation::Failure
    }
}

/// Observed result of running one fixture.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureOutcome {
    pub name: Symbol,
    pub expected_failure: bool,
    pub failed: bool,
    pub passed: bool,
    /// Soroban error code when the run failed, `0` otherwise.
    pub error_code: u32,
}

/// Success and failure scenarios every contributor can run locally.
///
/// Covers: a clean settlement, an oracle outage, an underfunded payer, an RPC
/// rejection, and an invalid amount.
pub fn standard_fixtures(env: &Env) -> Vec<ScenarioFixture> {
    let mut fixtures = Vec::new(env);

    fixtures.push_back(ScenarioFixture {
        name: Symbol::new(env, "oracle_happy_path"),
        expectation: ScenarioExpectation::Success,
        oracle_price: 1_000_000,
        oracle_failure: false,
        amount: 10_000,
        funding: 0,
        rpc_failure: false,
    });
    fixtures.push_back(ScenarioFixture {
        name: Symbol::new(env, "oracle_upstream_outage"),
        expectation: ScenarioExpectation::Failure,
        oracle_price: 1_000_000,
        oracle_failure: true,
        amount: 10_000,
        funding: 0,
        rpc_failure: false,
    });
    fixtures.push_back(ScenarioFixture {
        name: Symbol::new(env, "payer_underfunded"),
        expectation: ScenarioExpectation::Failure,
        oracle_price: 1_000_000,
        oracle_failure: false,
        amount: 10_000,
        funding: 5_000,
        rpc_failure: false,
    });
    fixtures.push_back(ScenarioFixture {
        name: Symbol::new(env, "rpc_rejected_settlement"),
        expectation: ScenarioExpectation::Failure,
        oracle_price: 1_000_000,
        oracle_failure: false,
        amount: 10_000,
        funding: 0,
        rpc_failure: true,
    });
    fixtures.push_back(ScenarioFixture {
        name: Symbol::new(env, "invalid_amount_guard"),
        expectation: ScenarioExpectation::Failure,
        oracle_price: 1_000_000,
        oracle_failure: false,
        amount: 0,
        funding: 1,
        rpc_failure: false,
    });

    fixtures
}

/// Run one fixture against a fresh, default-configured sandbox.
pub fn run_fixture(env: &Env, fixture: &ScenarioFixture) -> FixtureOutcome {
    run_fixture_with(env, &SandboxConfig::default(), fixture)
}

/// Run one fixture against an explicit configuration.
pub fn run_fixture_with(
    env: &Env,
    config: &SandboxConfig,
    fixture: &ScenarioFixture,
) -> FixtureOutcome {
    let expected_failure = fixture.expects_failure();
    match run_fixture_inner(env, config, fixture) {
        Ok(_) => FixtureOutcome {
            name: fixture.name.clone(),
            expected_failure,
            failed: false,
            passed: !expected_failure,
            error_code: 0,
        },
        Err(err) => FixtureOutcome {
            name: fixture.name.clone(),
            expected_failure,
            failed: true,
            passed: expected_failure,
            error_code: err as u32,
        },
    }
}

/// Run every fixture in [`standard_fixtures`], in declared order.
pub fn run_all_fixtures(env: &Env) -> Vec<FixtureOutcome> {
    let fixtures = standard_fixtures(env);
    let mut outcomes = Vec::new(env);
    let count = fixtures.len();
    for i in 0..count {
        let Some(fixture) = fixtures.get(i) else {
            continue;
        };
        outcomes.push_back(run_fixture(env, &fixture));
    }
    outcomes
}

fn run_fixture_inner(
    env: &Env,
    config: &SandboxConfig,
    fixture: &ScenarioFixture,
) -> Result<WorkflowReport, Error> {
    let mut sandbox = Sandbox::new(env, config.clone())?;
    let payer = Address::generate(env);
    let payee = Address::generate(env);
    let asset = Symbol::new(env, "sandbox_asset");

    if fixture.oracle_failure {
        sandbox.oracle_mut().fail_for(&asset);
    } else {
        sandbox
            .oracle_mut()
            .set_price(&asset, fixture.oracle_price)?;
    }
    sandbox.rpc_mut().set_fail_next(fixture.rpc_failure);
    sandbox.token_mut().mint(&payer, fixture.funding_amount())?;

    sandbox.run_primary_workflow(env, &payer, &payee, &asset, fixture.amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Ledger as _;

    fn happy_fixture(env: &Env) -> ScenarioFixture {
        ScenarioFixture {
            name: Symbol::new(env, "unit_happy"),
            expectation: ScenarioExpectation::Success,
            oracle_price: 2_500_000,
            oracle_failure: false,
            amount: 100_000,
            funding: 100_000,
            rpc_failure: false,
        }
    }

    // -- configuration guardrails -------------------------------------------

    #[test]
    fn mainnet_is_rejected() {
        let env = Env::default();
        let config = SandboxConfig {
            network: SandboxNetwork::Mainnet,
            ..SandboxConfig::default()
        };
        assert_eq!(config.validate(), Err(Error::ConfigInvalid));
        assert_eq!(Sandbox::new(&env, config).err(), Some(Error::ConfigInvalid));
    }

    #[test]
    fn bad_config_values_are_rejected() {
        let env = Env::default();
        let zero_seed = SandboxConfig {
            deterministic_seed: 0,
            ..SandboxConfig::default()
        };
        assert_eq!(zero_seed.validate(), Err(Error::ConfigInvalid));

        let slow = SandboxConfig {
            simulated_latency_ledgers: MAX_SIMULATED_LATENCY_LEDGERS + 1,
            ..SandboxConfig::default()
        };
        assert_eq!(slow.validate(), Err(Error::ConfigInvalid));

        assert_eq!(SandboxConfig::default().validate(), Ok(()));
        assert!(Sandbox::new(&env, SandboxConfig::new(42)).is_ok());
    }

    #[test]
    fn production_credentials_are_refused() {
        let env = Env::default();
        let sandbox = Sandbox::new(&env, SandboxConfig::default()).unwrap();
        // Sandbox runs need no secret at all.
        assert_eq!(sandbox.assert_credential_free(None), Ok(()));
        assert_eq!(sandbox.assert_credential_free(Some("   ")), Ok(()));
        // Any presented key material is refused.
        let seed = "SAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        assert_eq!(
            sandbox.assert_credential_free(Some(seed)),
            Err(Error::UnsafeSecret)
        );
        // Newly built sandboxes also accept no-secret runs through the guard.
        let checked = Sandbox::new_checked(&env, SandboxConfig::default(), None, None);
        assert!(checked.is_ok());
        let refused = Sandbox::new_checked(&env, SandboxConfig::default(), Some(seed), None);
        assert!(refused.is_err());
    }

    #[test]
    fn local_runs_are_loopback_only() {
        let env = Env::default();
        let sandbox = Sandbox::new(&env, SandboxConfig::default()).unwrap();
        assert_eq!(
            sandbox.assert_endpoint_allowed("http://localhost:8000/soroban/rpc"),
            Ok(())
        );
        assert_eq!(
            sandbox.assert_endpoint_allowed("http://127.0.0.1:8000"),
            Ok(())
        );
        assert_eq!(
            sandbox.assert_endpoint_allowed("https://soroban-mainnet.stellar.org"),
            Err(Error::ConfigInvalid)
        );
        assert_eq!(
            sandbox.assert_endpoint_allowed("http://has space/rpc"),
            Err(Error::ConfigInvalid)
        );

        let testnet = Sandbox::new(
            &env,
            SandboxConfig {
                network: SandboxNetwork::Testnet,
                ..SandboxConfig::default()
            },
        )
        .unwrap();
        assert_eq!(
            testnet.assert_endpoint_allowed("https://soroban-testnet.stellar.org"),
            Ok(())
        );
        assert_eq!(
            testnet.assert_endpoint_allowed("not-a-url"),
            Err(Error::ConfigInvalid)
        );
    }

    // -- adapter determinism ------------------------------------------------

    #[test]
    fn rng_is_reproducible_for_a_seed() {
        let mut a = DeterministicRng::new(7);
        let mut b = DeterministicRng::new(7);
        for _ in 0..8 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut c = DeterministicRng::new(8);
        assert_ne!(DeterministicRng::new(7).next_u64(), c.next_u64());
        // Range helper stays inside the requested window, including an empty one.
        let mut rng = DeterministicRng::new(DEFAULT_SEED);
        for _ in 0..16 {
            let v = rng.next_i128(5, 10);
            assert!((5..10).contains(&v));
        }
        assert_eq!(rng.next_i128(3, 3), 3);
    }

    #[test]
    fn adapters_return_deterministic_responses() {
        let env = Env::default();
        let asset = Symbol::new(&env, "sandbox_asset");

        let run = || {
            let mut sandbox = Sandbox::new(&env, SandboxConfig::new(99)).unwrap();
            sandbox.oracle_mut().set_price(&asset, 1_234_567).unwrap();
            let oracle = sandbox.oracle_mut();
            let first = oracle.quote(&env, &asset).unwrap();
            let second = oracle.quote(&env, &asset).unwrap();
            (first, second, oracle.quotes_served())
        };

        let (first_a, second_a, served_a) = run();
        let (first_b, second_b, served_b) = run();
        assert_eq!(first_a, first_b);
        assert_eq!(second_a, second_b);
        assert_eq!(served_a, 2);
        assert_eq!(served_a, served_b);
        // Same seed, same jitter draw from the sandbox RNG.
        let mut a = Sandbox::new(&env, SandboxConfig::new(99)).unwrap();
        let mut b = Sandbox::new(&env, SandboxConfig::new(99)).unwrap();
        assert_eq!(a.rng_mut().next_u64(), b.rng_mut().next_u64());
    }

    #[test]
    fn oracle_failure_and_unknown_assets_are_reported() {
        let env = Env::default();
        let asset = Symbol::new(&env, "sandbox_asset");
        let mut oracle = FakeOracleAdapter::new(&env, 1);
        assert_eq!(oracle.quote(&env, &asset), Err(Error::NotFound));
        assert_eq!(oracle.set_price(&asset, 0), Err(Error::InvalidAmount));
        oracle.set_price(&asset, 10).unwrap();
        oracle.fail_for(&asset);
        assert_eq!(oracle.quote(&env, &asset), Err(Error::NotFound));
        assert_eq!(oracle.quotes_served(), 0);
    }

    #[test]
    fn token_adapter_never_touches_real_wallets() {
        let env = Env::default();
        let mut token = FakeTokenAdapter::new(&env, 1);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        assert_eq!(token.balance_of(&alice), 0);
        assert_eq!(token.mint(&alice, 0), Err(Error::InvalidAmount));
        token.mint(&alice, 1_000).unwrap();
        assert_eq!(token.mint(&alice, i128::MAX), Err(Error::Overflow));
        assert_eq!(token.total_supply(), 1_000);

        assert_eq!(
            token.transfer(&alice, &bob, 2_000),
            Err(Error::InsufficientBalance)
        );
        token.transfer(&alice, &bob, 400).unwrap();
        assert_eq!(token.balance_of(&alice), 600);
        assert_eq!(token.balance_of(&bob), 400);
        assert_eq!(token.transfers(), 1);
    }

    #[test]
    fn rpc_adapter_keeps_submissions_in_memory() {
        let env = Env::default();
        let mut rpc = FakeRpcAdapter::new(&env, 100, 1_700_000_000, 2);
        assert_eq!(rpc.submit(&symbol_short!("settle")), Ok(102));
        assert_eq!(rpc.submitted_count(), 1);
        rpc.set_fail_next(true);
        assert_eq!(rpc.submit(&symbol_short!("settle")), Err(Error::Expired));
        assert_eq!(rpc.submitted_count(), 1);
        rpc.advance(10);
        assert_eq!(rpc.ledger(), 112);
        assert_eq!(rpc.timestamp(), 1_700_000_060);
    }

    // -- primary workflow ---------------------------------------------------

    #[test]
    fn primary_workflow_runs_locally() {
        let env = Env::default();
        let mut sandbox = Sandbox::new(&env, SandboxConfig::new(3)).unwrap();
        let payer = Address::generate(&env);
        let payee = Address::generate(&env);
        let asset = Symbol::new(&env, "sandbox_asset");
        sandbox.oracle_mut().set_price(&asset, 500_000).unwrap();
        sandbox.token_mut().mint(&payer, 100_000).unwrap();

        let report = sandbox
            .run_primary_workflow(&env, &payer, &payee, &asset, 100_000)
            .unwrap();

        assert_eq!(report.steps, 4);
        assert_eq!(report.amount, 100_000);
        assert_eq!(report.fee, 500); // 50 bps
        assert_eq!(report.oracle_price, 500_000);
        assert_eq!(report.payee_balance, 99_500);
        // The fee stays with the payer — the sandbox has no treasury leg, so
        // the workflow can never move value it was not funded with.
        assert_eq!(report.payer_balance, 500);
        assert!(report.credential_free);
        assert!(report.settled_ledger > sandbox.config().start_ledger);
        assert_eq!(sandbox.token().transfers(), 1);
        assert_eq!(sandbox.rpc().submitted_count(), 1);
        // Every adapter hop is traced for later assertions.
        assert_eq!(sandbox.calls().len(), 4);
    }

    #[test]
    fn primary_workflow_guards_bad_inputs() {
        let env = Env::default();
        let mut sandbox = Sandbox::new(&env, SandboxConfig::default()).unwrap();
        let payer = Address::generate(&env);
        let payee = Address::generate(&env);
        let asset = Symbol::new(&env, "sandbox_asset");
        sandbox.oracle_mut().set_price(&asset, 1).unwrap();

        assert_eq!(
            sandbox.run_primary_workflow(&env, &payer, &payee, &asset, 0),
            Err(Error::InvalidAmount)
        );
        assert_eq!(
            sandbox.run_primary_workflow(&env, &payer, &payee, &asset, 10),
            Err(Error::InsufficientBalance)
        );
        assert_eq!(sandbox.rpc().submitted_count(), 0);
    }

    #[test]
    fn fee_estimate_rejects_overflow() {
        assert_eq!(estimate_fee(100_000), Ok(500));
        assert_eq!(estimate_fee(i128::MAX), Err(Error::Overflow));
    }

    // -- fixtures -----------------------------------------------------------

    #[test]
    fn standard_fixtures_cover_success_and_failure() {
        let env = Env::default();
        let fixtures = standard_fixtures(&env);
        assert_eq!(fixtures.len(), 5);
        let mut successes = 0;
        let mut failures = 0;
        for i in 0..fixtures.len() {
            let fixture = fixtures.get(i).unwrap();
            if fixture.expects_failure() {
                failures += 1;
            } else {
                successes += 1;
            }
        }
        assert_eq!(successes, 1);
        assert_eq!(failures, 4);
    }

    #[test]
    fn every_standard_fixture_matches_its_expectation() {
        let env = Env::default();
        let outcomes = run_all_fixtures(&env);
        assert_eq!(outcomes.len(), 5);
        for i in 0..outcomes.len() {
            let outcome = outcomes.get(i).unwrap();
            assert!(outcome.passed, "fixture did not match expectation");
            assert_eq!(outcome.failed, outcome.expected_failure);
        }
    }

    #[test]
    fn fixture_outcomes_are_reproducible_across_runs() {
        let env = Env::default();
        let first = run_all_fixtures(&env);
        let second = run_all_fixtures(&env);
        assert_eq!(first.len(), second.len());
        for i in 0..first.len() {
            assert_eq!(first.get(i), second.get(i));
        }
    }

    #[test]
    fn fixture_helpers_and_expectation_mismatch_are_visible() {
        let env = Env::default();
        let fixture = happy_fixture(&env);
        assert_eq!(fixture.funding_amount(), 100_000);
        assert!(!fixture.expects_failure());
        let outcome = run_fixture(&env, &fixture);
        assert!(outcome.passed);
        assert!(!outcome.failed);
        assert_eq!(outcome.error_code, 0);

        // A fixture that claims failure but succeeds is reported as failed,
        // which is how a regression would surface.
        let wrong = ScenarioFixture {
            expectation: ScenarioExpectation::Failure,
            ..fixture.clone()
        };
        let outcome = run_fixture(&env, &wrong);
        assert!(!outcome.passed);
        assert!(outcome.expected_failure);
        assert!(!outcome.failed);
    }

    #[test]
    fn ledger_helpers_are_available_for_fixture_timing() {
        let env = Env::default();
        env.ledger().set_timestamp(1_800_000_000);
        let mut sandbox = Sandbox::new(&env, SandboxConfig::default()).unwrap();
        sandbox.rpc_mut().advance(LEDGERS_PER_DAY);
        assert_eq!(sandbox.rpc().ledger(), 1 + LEDGERS_PER_DAY);
    }
}
