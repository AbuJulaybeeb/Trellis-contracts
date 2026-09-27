# Testing & Simulation Module

A comprehensive testing framework for all contracts in the trellis-contracts repository. This module provides mocks, helpers, simulation tools, and fuzzing harnesses to thoroughly test all protocol functionality.

## Overview

The module is organized into 5 main components:
- `mocks/` - Mock implementations of external dependencies (tokens, oracles, registry)
- `helpers/` - Time manipulation, environment setup, and debugging utilities
- `simulation/` - Deterministic transaction simulation and gas profiling
- `fuzzing/` - Fuzzing harnesses for critical modules (Access Control, Payments, Upgradeability)
- `examples/` - Example test suites demonstrating usage of all features

## Adding to Your Project

Add the testing module to your contract's `Cargo.toml`:

```toml
[dependencies]
testing = { path = "../testing", features = ["testutils"] }
```

## Quick Start

### Basic Test Environment Setup

```rust
use testing::helpers::*;
use testing::mocks::*;

#[test]
fn my_contract_test() {
    // Create a test environment with 5 additional users
    let mut test_env = TestEnvironment::new(5);
    
    // Create a token for testing
    let (token_addr, token_client, asset_client) = test_env.create_stellar_token("usdc");
    
    // Mint initial balances to all users
    test_env.mint_tokens_to_users(&asset_client, &10_000_000);
    
    // Register your contract
    let my_contract_addr = test_env.register_contract("my_contract", MyContract);
    
    // Get a user
    let alice = test_env.user(0);
    
    // Run your test...
}
```

### Time Manipulation

```rust
use testing::helpers::*;

// Advance ledger sequence by 100 blocks
advance_ledger_sequence(&env, 100);

// Advance time by 24 hours (86400 seconds)
advance_ledger_time(&env, 86400);

// Jump to a specific timestamp
set_ledger_timestamp(&env, 1704067200); // 2024-01-01
```

### Mocks for External Dependencies

```rust
use testing::mocks::*;

// Create a mock ERC20-like token
let (token_addr, token_client) = create_mock_token(
    &env, 
    &admin, 
    6,      // decimals
    "USD Coin", 
    "USDC"
);

// Create a mock price oracle
let (oracle_addr, _) = create_mock_oracle(&env, &admin);

// Update price in oracle
let oracle_client = MockOracleClient::new(&env, &oracle_addr);
oracle_client.update_price(&btc_address, &450000000000, &6); // $45,000 with 6 decimals

// Create a mock contract registry
let registry_addr = create_mock_registry(&env, &admin);
```

## Integration Sandbox Mode

Run the primary workflow locally against fake external services — no
production credentials, real wallets, or irreversible records:

```rust
use testing::sandbox::*;

let mut sandbox = Sandbox::new(&env, SandboxConfig::new(1234)).unwrap();
let asset = soroban_sdk::Symbol::new(&env, "sandbox_asset");

sandbox.oracle_mut().set_price(&asset, 1_000_000).unwrap();
sandbox.token_mut().mint(&payer, 100_000).unwrap();

let report = sandbox
    .run_primary_workflow(&env, &payer, &payee, &asset, 100_000)
    .unwrap();
assert_eq!(report.fee, 500); // 50 bps

// Or run the whole declared scenario matrix:
for outcome in run_all_fixtures(&env).iter() {
    assert!(outcome.passed, "fixture expectation not met");
}
```

Mainnet is rejected, local runs are loopback-only, key material is refused,
and every adapter response is reproducible from `deterministic_seed`. See
[`docs/SANDBOX.md`](../docs/SANDBOX.md) for guardrails, fixtures, and
limitations.

## Simulation & Gas Profiling

Run deterministic simulations of complex scenarios:

```rust
use testing::simulation::*;

let mut simulator = DeterministicSimulator::new();

// Execute transactions in simulation
for i in 0..100 {
    simulator.execute_tx::<()>(
        &format!("transfer_{}", i),
        &token_address,
        "transfer",
        &sender,
        (recipient.clone(), amount).into(),
    ).unwrap();
}

// Generate report
let results = simulator.finalize();
results.print_gas_report();
```

## Fuzzing Critical Modules

### Access Control Fuzzing

```rust
use testing::fuzzing::*;

let mut fuzzer = AccessControlFuzzer::new(&env, Some(12345)); // fixed seed for reproducibility
let results = fuzzer.fuzz(&AccessControlFuzzConfig::default());
results.print_summary();
assert!(results.caught_violations == results.unauthorized_attempts);
```

### Payment/Treasury Fuzzing

```rust
let mut fuzzer = PaymentsFuzzer::new(&env, None);
let results = fuzzer.fuzz(&PaymentsFuzzConfig::default());
assert!(results.invariant_violations.is_empty(), "No invariant violations allowed");
```

### Upgradeability Fuzzing

```rust
let mut fuzzer = UpgradeabilityFuzzer::new(&env, Some(98765));
let results = fuzzer.fuzz(&UpgradeabilityFuzzConfig::default());
assert_eq!(results.unauthorized_attempts_blocked, results.failed_attempts);
```

## Run All Tests

```bash
cd testing
cargo test
```

## CI Integration

The example tests in this module run automatically in CI. Add your contract's tests that depend on this module to ensure they're always tested against the latest testing utilities.

## Best Practices

1. Always use `TestEnvironment` for consistent setup across all tests
2. Use the simulation module for complex multi-step workflows
3. Add fuzzing tests for all critical functions in your contract
4. Profile gas usage of your contract's operations using the GasProfiler
5. Use the mock contracts to isolate your contract's logic during testing
6. Use sandbox mode when a test needs an oracle, token, or RPC round-trip:
   the fakes are deterministic, so failures are reproducible