# Integration Sandbox Mode (Issue #57)

Sandbox mode runs the primary Trellis workflow locally against **fake**
external services — oracle, token, and RPC — so contributors can exercise
integration paths without production credentials, real wallets, or
irreversible records.

The implementation lives in the `testing` crate:
[`testing/src/sandbox.rs`](../testing/src/sandbox.rs).

## Why

Contract tests are usually happy-path only: a real oracle, a real token, or a
real RPC endpoint is not available to a contributor, and pointing tests at
production is both unsafe and unreviewable. Sandbox mode fixes that by making
every external dependency an explicit, deterministic fake.

## Quick start

```rust
use testing::sandbox::*;

let env = soroban_sdk::Env::default();
let mut sandbox = Sandbox::new(&env, SandboxConfig::new(1234)).unwrap();

let payer = soroban_sdk::Address::generate(&env);
let payee = soroban_sdk::Address::generate(&env);
let asset = soroban_sdk::Symbol::new(&env, "sandbox_asset");

sandbox.oracle_mut().set_price(&asset, 1_000_000).unwrap();
sandbox.token_mut().mint(&payer, 100_000).unwrap();

let report = sandbox
    .run_primary_workflow(&env, &payer, &payee, &asset, 100_000)
    .unwrap();

assert_eq!(report.fee, 500); // 50 bps
assert_eq!(sandbox.rpc().submitted_count(), 1);
```

Run the fixtures instead when you want the whole scenario matrix:

```rust
let outcomes = run_all_fixtures(&env);
for i in 0..outcomes.len() {
    assert!(outcomes.get(i).unwrap().passed);
}
```

## Guardrails

| Rule | Enforced by | Failure |
|---|---|---|
| Mainnet is never targeted | `SandboxConfig::validate` | `Error::ConfigInvalid` |
| Zero seed / zero start ledger / absurd latency | `SandboxConfig::validate` | `Error::ConfigInvalid` |
| No key material at all in a sandbox run | `Sandbox::assert_credential_free` | `Error::UnsafeSecret` |
| Local runs only talk to loopback | `Sandbox::assert_endpoint_allowed` | `Error::ConfigInvalid` |
| A hand-built plan cannot delete frozen data | retention module (Issue #45) | reported, not deleted |

Build a guarded sandbox with `Sandbox::new_checked(env, config, secret, endpoint)`
— it applies the credential and endpoint rules up front.

Loopback prefixes allowed for `SandboxNetwork::Local`:
`http://localhost`, `http://127.0.0.1`, `http://[::1]`.

## Determinism

Nothing in the sandbox reads wall-clock time or OS entropy. The only
variability is `DeterministicRng`, seeded from
`SandboxConfig::deterministic_seed`, so two runs of the same fixture produce
the same values. `FixtureOutcome` values are therefore safe to snapshot and
compare across runs (see `fixture_outcomes_are_reproducible_across_runs`).

`SandboxConfig::default()` uses `DEFAULT_SEED` and a start ledger of `1`, with
`simulated_latency_ledgers = 1`.

## Adapters

| Adapter | Replaces | Deterministic behaviour | Failure modes |
|---|---|---|---|
| `FakeOracleAdapter` | Price oracle / external feed | Fixed prices per asset, seeded quotes | Unknown asset and `fail_for` → `Error::NotFound`; non-positive price → `Error::InvalidAmount` |
| `FakeTokenAdapter` | SAC / token contract | In-process balances, no ledger writes | Insufficient balance → `Error::InsufficientBalance`; overflow → `Error::Overflow` |
| `FakeRpcAdapter` | Soroban RPC | Submitted ops stay in an in-memory log, simulated latency | `set_fail_next(true)` → `Error::Expired` |

Each hop through an adapter is appended to `Sandbox::calls()`
(`AdapterCall { adapter, operation, allowed }`), so a test can assert not only
the result but the path taken — including calls the guards refused
(`allowed == false`).

## Fixtures

`standard_fixtures()` ships one success and four failure scenarios:

| Fixture | Expectation | What it covers |
|---|---|---|
| `oracle_happy_path` | Success | Clean quote → transfer → settlement |
| `oracle_upstream_outage` | Failure | Oracle unavailable (`Error::NotFound`) |
| `payer_underfunded` | Failure | Insufficient balance mid-workflow |
| `rpc_rejected_settlement` | Failure | Inclusion timeout (`Error::Expired`) |
| `invalid_amount_guard` | Failure | Non-positive amount rejected before any side effect |

`run_fixture` reports whether the observed result matched the declaration
(`FixtureOutcome::passed`), so a regression that turns a declared failure into
a success — or vice versa — is visible.

## Local setup

No production configuration is required. The sandbox needs no secret key, no
RPC URL, and no network access; `Env::default()` plus the fakes is enough.

```bash
# from the repository root — the testing crate is excluded from the workspace
cargo test --manifest-path testing/Cargo.toml sandbox

# or from inside the crate
cd testing && cargo test sandbox
```

## Limitations

- The sandbox fakes **external** services, not contract logic. Soroban
  host/ledger semantics (storage TTLs, auth, budgets) come from the real
  `Env` returned by `Env::default()`.
- `FakeRpcAdapter` does not model fee bumps, sequence collisions, or ledger
  close races; latency is a fixed per-operation advance.
- Adapters hold state in memory only. Nothing they do is observable after the
  test process exits, which is the point — but it also means they cannot be
  used as an integration environment for a deployed contract.
- `SandboxNetwork::Testnet` validates the endpoint shape only. It does not
  perform network calls; use it for configuration checks, and keep real
  testnet runs behind your normal review process.
- Determinism is per-seed. Changing `deterministic_seed`, the ordering of
  fixture declarations, or the ledger start values changes the drawn values
  by design.
