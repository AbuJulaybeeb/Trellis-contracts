<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/png/trellis-lockup-dark-320.png">
    <img src="brand/png/trellis-lockup-320.png" alt="Trellis" width="331">
  </picture>
</p>

<h1 align="center">Trellis Smart Contracts</h1>

<p align="center">
  <strong>Open-source Soroban smart contracts powering transparent, secure and verifiable<br>humanitarian aid distribution on the Stellar blockchain.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-1C6B55?style=flat-square" alt="License">
  <img src="https://img.shields.io/badge/Rust-1.80%2B-14201C?style=flat-square" alt="Rust">
  <img src="https://img.shields.io/badge/Soroban-latest-1C6B55?style=flat-square" alt="Soroban">
  <img src="https://img.shields.io/badge/Stellar-blockchain-14201C?style=flat-square" alt="Stellar">
  <img src="https://img.shields.io/badge/coverage-TBD-1C6B55?style=flat-square" alt="Coverage">
  <img src="https://img.shields.io/badge/status-active%20development-E39A3C?style=flat-square" alt="Status">
</p>

<p align="center">
  <a href="https://trellis-stellar.github.io/Trellis-contracts/" style="text-decoration: none; color: #1C6B55; font-weight: 500;">📖 API Documentation</a>
</p>

---

# Table of Contents

- Overview
- Why Smart Contracts
- Core Features
- Architecture
- Contract Modules
- Technology Stack
- Deployments
- Development Setup
- Build
- Testing
- Deployment
- Security
- Upgrade Strategy
- Contract Interfaces
- Events
- Documentation

## Documentation

| Document | Description |
| --- | --- |
| [UPGRADEABILITY.md](UPGRADEABILITY.md) | Contract upgrade model and migration strategy |
| [SECURITY_BATCH.md](SECURITY_BATCH.md) | Security audit findings and remediation status |
| [GAS_OPTIMIZATION.md](GAS_OPTIMIZATION.md) | Gas optimization patterns and benchmarks |
| [testing/README.md](testing/README.md) | Testing harness setup and conventions |
| [shared/README.md](shared/README.md) | Shared library utilities and helpers |
| [security/README.md](security/README.md) | Security model and access control overview |
- Storage Layout
- Documentation
- Contribution Guide
- Brand
- License

---

# Overview

The **Trellis Contracts** repository contains every on-chain component responsible for securely executing humanitarian aid transactions on the Stellar blockchain using **Soroban**.

These contracts serve as the trust layer of the platform.

Instead of relying on centralized intermediaries, every payment, verification, referral reward, and settlement is executed transparently on-chain.

The contracts are designed to be:

- Secure
- Auditable
- Upgradeable
- Gas Efficient
- Modular
- Production Ready

---

# Why Smart Contracts?

Traditional donation systems rely heavily on centralized infrastructure.

Trellis replaces this model by executing critical operations directly on-chain.

The contracts provide:

- Transparent aid settlements
- Immutable transaction records
- Referral commission distribution
- Treasury management
- Multi-signature administration
- Emergency controls
- Upgrade governance

Every important financial action becomes publicly verifiable.

---

# Core Features

## Direct Aid Settlement

Transfers donor funds directly to verified recipients.

Features

- Atomic transfers
- Escrow support
- Expiration handling
- Claim verification
- Replay protection

---

## Claim Links

Generate secure claim identifiers.

Supports

- One-time claims
- Expiration dates
- Maximum claims
- Signature validation
- Hash verification

---

## Referral Rewards

Automatically distributes affiliate commissions.

Supports

- Multi-tier referrals
- Percentage configuration
- Reward limits
- Treasury payouts

---

## Treasury

Central treasury responsible for:

- Holding protocol reserves
- Reward distribution
- Administrative transfers
- Emergency withdrawals

---

## Governance

Administrative controls include

- Contract upgrades
- Parameter updates
- Treasury permissions
- Emergency pause
- Role assignments

---

## Identity Verification

Stores verification references without exposing private user information.

Supports

- Hash verification
- Metadata pointers
- AI verification references
- Off-chain oracle integration

---

# Smart Contract Architecture

```

┌────────────────────────────┐
│        Frontend            │
└────────────┬───────────────┘
│
▼
┌────────────────────────────┐
│       Backend API          │
└────────────┬───────────────┘
│
▼
┌────────────────────────────┐
│      Soroban Contracts     │
├────────────────────────────┤
│ Aid Contract               │
│ Treasury Contract          │
│ Referral Contract          │
│ Governance Contract        │
│ Oracle Contract            │
│ Registry Contract          │
└────────────┬───────────────┘
│
▼
Stellar Ledger

```

---

# Contract Modules

## aid-contract

Responsible for

- Aid creation
- Aid claiming
- Settlement
- Escrow
- Refunds

---

## treasury-contract

Responsible for

- Treasury balances
- Reward distribution
- Protocol funds
- Emergency reserve

---

## referral-contract

Responsible for

- Referral registration
- Commission calculations
- Tier rewards
- Reward claims

---

## governance-contract

Responsible for

- Upgrade authorization
- Admin roles
- Parameter management
- Contract registry

---

## oracle-contract

Responsible for

- AI verification references
- External signatures
- Verification proofs

---

## registry-contract

Responsible for

- Contract discovery
- Address registry
- Version tracking

---

## shared

Shared library containing types and utilities reused by every contract in the workspace. See [shared/README.md](shared/README.md) for error codes, reserved ranges, and event schemas.

---

## testing

Comprehensive testing framework with mocks, helpers, simulation tools, and fuzzing harnesses. See [testing/README.md](testing/README.md) for usage examples and available utilities.

---

## security

Security audit tooling and CI gating for static analysis and vulnerability scanning. See [security/README.md](security/README.md) for running audits and managing allowlists.

---

# Technology Stack

| Technology     | Purpose                 |
| -------------- | ----------------------- |
| Rust           | Smart contract language |
| Soroban SDK    | Contract development    |
| Stellar CLI    | Deployment              |
| Cargo          | Package manager         |
| Soroban RPC    | Network interaction     |
| GitHub Actions | CI/CD                   |

---

# Deployments

For deployed contract addresses, networks, and versions, see [DEPLOYMENTS.md](./DEPLOYMENTS.md).

### Quick Reference

| Network     | Status           | Details                                          |
| ----------- | ---------------- | ------------------------------------------------ |
| **Testnet** | Not yet deployed | See [DEPLOYMENTS.md](./DEPLOYMENTS.md) for setup |
| **Mainnet** | Not yet deployed | Awaiting security review and governance          |

### Deploying Contracts

To deploy all contracts to testnet:

```bash
./scripts/deploy.sh testnet
./scripts/record-deployments.sh testnet
```

Then verify:

```bash
./scripts/verify.sh testnet
```

For mainnet deployments, see [UPGRADEABILITY.md](./UPGRADEABILITY.md) for the full upgrade governance process.

### Upgrade History

All upgrades are tracked in the on-chain upgradeability registry. To view upgrade history for a contract:

```bash
soroban contract invoke \
  --id <UPGRADEABILITY_CONTRACT_ID> \
  -- get_upgrade_history \
  --contract-id <CONTRACT_ID>
```

---# Project Structure

```

contracts/

├── aid-contract/
├── treasury-contract/
├── referral-contract/
├── governance-contract/
├── oracle-contract/
├── registry-contract/
│
├── shared/
│ ├── errors.rs
│ ├── events.rs
│ ├── storage.rs
│ ├── auth.rs
│ ├── math.rs
│ └── utils.rs
│
├── scripts/
│ ├── deploy.sh
│ ├── upgrade.sh
│ ├── initialize.sh
│ └── verify.sh
│
├── tests/
├── Cargo.toml
├── Cargo.lock
└── README.md

```

---

# Development Setup

## Install Rust

```
rustup update
```

Install Soroban CLI

```
cargo install --locked soroban-cli
```

Clone repository

```
git clone https://github.com/TRELLIS-STELLAR/Trellis.git

cd Trellis
```

---

# Build

```
cargo build --release
```

Build optimized WASM

```
cargo build \
--target wasm32v1-none \
--release
```

---

# Testing

Run all tests

```
cargo test
```

Run integration tests

```
cargo test --test integration
```

Generate coverage

```
cargo llvm-cov
```

---

# Deployment

Deploy to Testnet

```
soroban contract deploy \
--wasm target/wasm32v1-none/release/aid_contract.wasm \
--source admin
```

Initialize

```
soroban contract invoke \
--id CONTRACT_ID \
-- initialize
```

---

# Security

The contracts implement

- Reentrancy protection
- Signature verification
- Overflow-safe arithmetic
- Access control
- Replay protection
- Input validation
- Treasury limits
- Emergency pause
- Time-based expirations
- Storage validation

---

# Upgrade Strategy

Supports controlled upgrades through governance.

Only authorized administrators may:

- Upgrade contracts
- Register new implementations
- Pause protocol
- Resume protocol
- Update treasury
- Modify protocol parameters

---

# Storage Layout

Persistent storage includes

Aid Records

- Aid ID
- Donor
- Recipient
- Amount
- Status
- Timestamp

Referral Records

- Wallet
- Referrer
- Commission
- Tier

Treasury

- Balance
- Rewards
- Fees

Governance

- Admins
- Roles
- Versions
- Registry

---

# Contract Interfaces

## aid-contract

Core aid disbursement and escrow contract. Recipients claim funds within expiry windows; expired aids are refundable to donors.

### Lifecycle

#### `initialize`

Initialize the contract (required before other calls; admin only).

```
initialize(
  admin: Address,
  treasury: Address,
  token: Address,
  default_expiry_secs: u64
) -> Result<(), Error>
```

- **Parameters**
  - `admin` — Admin address with governance privileges
  - `treasury` — Treasury address for fees or emergency withdrawals
  - `token` — Accepted escrow token address
  - `default_expiry_secs` — Default expiration time in seconds for new aids

- **Authorization** — `admin` must invoke (`require_auth()`)

- **Errors**
  - `Error::AlreadyInitialized` — Contract was already initialized

- **Events Emitted**
  - `ModuleInitialized` — `("logging", "initialized")` with module name, version, caller, timestamp

#### `get_admin`

Get the current admin address.

```
get_admin(env: Env) -> Address
```

- **Returns** — Current admin address

#### `get_treasury`

Get the treasury address.

```
get_treasury(env: Env) -> Option<Address>
```

- **Returns** — Treasury address or `None` if not set

#### `get_token`

Get the accepted escrow token.

```
get_token(env: Env) -> Option<Address>
```

- **Returns** — Token address or `None` if not set

#### `get_default_expiry`

Get the default expiry in seconds.

```
get_default_expiry(env: Env) -> Option<u64>
```

- **Returns** — Expiry duration in seconds or `None`

#### `is_initialized`

Check if the contract is initialized.

```
is_initialized(env: Env) -> bool
```

- **Returns** — `true` if initialized, `false` otherwise

#### `update_config`

Update configuration (admin only).

```
update_config(
  admin: Address,
  treasury: Option<Address>,
  default_expiry_secs: Option<u64>
) -> Result<(), Error>
```

- **Parameters**
  - `admin` — Must be the current admin
  - `treasury` — New treasury address (`None` = no change)
  - `default_expiry_secs` — New default expiry (`None` = no change)

- **Authorization** — `admin` must invoke (`require_auth()`)

- **Errors**
  - `Error::Unauthorized` — Caller is not the current admin

### Aid Creation

#### `create_aid`

Create a new aid disbursement and escrow funds from the donor.

```
create_aid(
  donor: Address,
  recipient: Address,
  amount: i128,
  expiry_ledger: u32
) -> u64
```

- **Parameters**
  - `donor` — Sender of the aid (must have token balance)
  - `recipient` — Intended claimant
  - `amount` — Amount in token units (must be > 0)
  - `expiry_ledger` — Ledger sequence after which aid cannot be claimed (must be > current)

- **Authorization** — `donor` must invoke (`require_auth()`)

- **Returns** — Newly assigned aid ID

- **Errors (panic)**
  - `Error::InvalidAmount` — Amount is ≤ 0
  - `AidError::NotExpiredYet` — Expiry ledger is ≤ current ledger

- **Token Transfer** — Transfers `amount` from donor to contract

- **Events Emitted**
  - `AidCreated` — `("aid", "created")` with (aid_id, donor, recipient, amount, created_at, expires_at)
  - Generic `AID_CREATED` legacy event

### Aid Claiming

#### `claim_aid`

Claim a pending aid and transfer funds to recipient.

```
claim_aid(
  aid_id: u64,
  recipient: Address
) -> Result<(), AidError>
```

- **Parameters**
  - `aid_id` — ID of the aid to claim
  - `recipient` — Must be the intended recipient

- **Authorization** — `recipient` must invoke (`require_auth()`)

- **Errors**
  - `AidError::Paused` — Contract is paused
  - `AidError::NotFound` — Aid ID does not exist
  - `AidError::AlreadyClaimed` — Aid was already claimed or refunded
  - `AidError::Expired` — Expiry ledger has passed (cannot claim expired aids)
  - `AidError::Unauthorized` — Caller is not the intended recipient

- **State Changes** — Updates aid status from `Pending` to `Settled`

- **Token Transfer** — Transfers `amount` from contract to recipient

- **Events Emitted**
  - `AidClaimed` — `("aid", "claimed")` with (aid_id, claimant, claimed_at)
  - `AidSettled` — `("aid", "settled")` with (aid_id, recipient, amount, settled_at)
  - Generic `AID_CLAIMED` and `AID_SETTLED` legacy events

### Refunds

#### `refund_aid`

Refund an expired, unclaimed aid to the original donor.

```
refund_aid(aid_id: u64) -> Result<(), AidError>
```

- **Parameters**
  - `aid_id` — ID of the expired aid to refund

- **Errors**
  - `AidError::NotFound` — Aid ID does not exist
  - `AidError::AlreadyClaimed` — Aid was already claimed (status is `Settled`)
  - `AidError::AlreadyRefunded` — Aid was already refunded
  - `AidError::NotExpiredYet` — Expiry ledger has not passed yet

- **State Changes** — Updates aid status from `Pending` to `Refunded`

- **Token Transfer** — Transfers `amount` from contract back to donor

- **Events Emitted**
  - `AidRefunded` — `("aid", "refunded")` with (aid_id, donor, amount, refunded_at)
  - Generic `AID_REFUNDED` legacy event

### Queries

#### `get_aid`

Get a single aid record by ID.

```
get_aid(env: Env, aid_id: u64) -> Option<AidRecord>
```

- **Parameters**
  - `aid_id` — ID to fetch

- **Returns** — Aid record with id, donor, recipient, token, amount, expiry_ledger, status or `None` if not found

#### `list_aids`

List all aids with pagination.

```
list_aids(env: Env, limit: u32, cursor: Option<u64>) -> PaginatedAidsResponse
```

- **Parameters**
  - `limit` — Max results per page
  - `cursor` — Start position (aid ID); `None` starts at 0

- **Returns** — `PaginatedAidsResponse { aids: Vec<AidRecord>, next_cursor: Option<u64> }`
  - Pass `next_cursor` back as `cursor` to fetch the next page
  - `next_cursor = None` means end of results

#### `list_aids_by_donor`

List aids created by a specific donor, with pagination.

```
list_aids_by_donor(
  env: Env,
  donor: Address,
  limit: u32,
  cursor: Option<u64>
) -> PaginatedAidsResponse
```

- **Parameters**
  - `donor` — Filter by donor address
  - `limit` — Max results per page
  - `cursor` — Start position (aid ID); `None` starts at 0

- **Returns** — `PaginatedAidsResponse { aids: Vec<AidRecord>, next_cursor: Option<u64> }`

#### Permission-aware discovery search

`search_aids(viewer, cursor, limit)` returns only pending, visible records for
which `viewer` authenticates as the donor, recipient, contract admin, or an
address granted access by the donor. Completed, hidden, and deleted records
are removed from the derived search index. Administrators can run
`repair_search_index(admin)` to rebuild that index from canonical aid storage;
the return value reports indexed, added, and removed entries. No migration is
required: older records are included on the next repair run.

### Admin Controls

#### `set_paused`

Pause or resume the contract (admin only).

```
set_paused(env: Env, admin: Address, paused: bool)
```

- **Parameters**
  - `admin` — Must be the current admin
  - `paused` — `true` to pause, `false` to resume

- **Authorization** — `admin` must invoke (`require_auth()`)

- **Effect** — When paused, `claim_aid` returns `AidError::Paused`

- **Events Emitted**
  - `PermissionChanged` — `("logging", "permission")` with (module, role, subject, granted, changed_at)

---

## treasury-contract

Protocol treasury management. Manages per-category balances, enforces withdrawal limits, and distributes referral rewards.

### Initialization

#### `initialize`

Initialize the treasury contract (required before other calls; admin only).

```
initialize(
  admin: Address,
  max_withdrawal_limit: i128
) -> Result<(), Error>
```

- **Parameters**
  - `admin` — Admin address with governance privileges
  - `max_withdrawal_limit` — Max amount per withdrawal transaction (must be > 0)

- **Authorization** — `admin` must invoke (`require_auth()`)

- **Errors**
  - `Error::InvalidArgument` — Limit is ≤ 0

- **Initial State** — Admin is automatically granted `TreasuryManager` role

- **Events Emitted**
  - `ModuleInitialized` — `("logging", "initialized")` with module name, version, caller, timestamp

### Role Management

#### `add_treasury_manager`

Grant the `TreasuryManager` role to an address (admin only).

```
add_treasury_manager(
  caller: Address,
  who: Address
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `who` — Address to grant manager role

- **Authorization** — `caller` must be admin

- **Errors**
  - `Error::Unauthorized` — Caller is not admin

- **Events Emitted**
  - `PermissionChanged` — `("logging", "permission")` with (treasury, manager, who, true, changed_at)

#### `remove_treasury_manager`

Revoke the `TreasuryManager` role from an address (admin only).

```
remove_treasury_manager(
  caller: Address,
  who: Address
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `who` — Address to revoke manager role

- **Authorization** — `caller` must be admin

- **Errors**
  - `Error::Unauthorized` — Caller is not admin

- **Events Emitted**
  - `PermissionChanged` — `("logging", "permission")` with (treasury, manager, who, false, changed_at)

### Configuration

#### `set_withdrawal_limit`

Update the max per-transaction withdrawal limit (admin only).

```
set_withdrawal_limit(
  caller: Address,
  new_limit: i128
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `new_limit` — New max withdrawal amount (must be > 0)

- **Authorization** — `caller` must be admin

- **Errors**
  - `Error::Unauthorized` — Caller is not admin
  - `Error::InvalidArgument` — Limit is ≤ 0

- **Events Emitted**
  - `ActionExecuted` — `("logging", "action")` with (treasury, wd_limit, caller, true, timestamp)

#### `withdrawal_limit`

Get the current max per-transaction withdrawal limit.

```
withdrawal_limit(env: Env) -> i128
```

- **Returns** — Max withdrawal amount in tokens

### Deposits

#### `deposit`

Credit funds into a category balance (treasury manager only).

```
deposit(
  caller: Address,
  category: Symbol,
  amount: i128
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must hold `TreasuryManager` role
  - `category` — Category to credit (e.g., `reserve`, `rewards`)
  - `amount` — Amount to add (must be > 0)

- **Authorization** — `caller` must be a treasury manager

- **Errors**
  - `Error::InvalidArgument` — Amount is ≤ 0
  - `Error::Unauthorized` — Caller is not a treasury manager
  - `Error::Overflow` — Adding amount would overflow balance

- **State Changes** — Increments category balance

- **Events Emitted**
  - `TreasuryDeposit` — `("treasury", "deposit")` with (category, caller, amount, new_balance)
  - `ActionExecuted` — `("logging", "action")` with (treasury, deposit, caller, true, timestamp)

### Withdrawals

#### `withdraw`

Withdraw from a category to a recipient (treasury manager only).

```
withdraw(
  caller: Address,
  to: Address,
  amount: i128,
  category: Symbol
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must hold `TreasuryManager` role
  - `to` — Recipient address
  - `amount` — Withdrawal amount (must be > 0)
  - `category` — Category to withdraw from

- **Authorization** — `caller` must be a treasury manager

- **Validation Order** (cheap-first for gas optimization)
  1. `amount > 0`
  2. `amount ≤ withdrawal_limit`
  3. `amount ≤ category balance`
  4. `caller` has `TreasuryManager` role

- **Errors**
  - `Error::InvalidArgument` — Amount is ≤ 0
  - `Error::WithdrawalLimitExceeded` — Amount exceeds per-tx limit
  - `Error::InsufficientBalance` — Category balance insufficient
  - `Error::Unauthorized` — Caller is not a treasury manager

- **State Changes** — Decrements category balance

- **Events Emitted**
  - `TreasuryWithdrawal` — `("treasury", "withdraw")` with (category, to, amount, remaining_balance)
  - `ActionExecuted` — `("logging", "action")` with (treasury, withdraw, caller, true, timestamp)

#### `category_balance`

Get the current balance for a category.

```
category_balance(env: Env, category: Symbol) -> i128
```

- **Parameters**
  - `category` — Category to query

- **Returns** — Balance (0 if never funded)

#### `emergency_withdraw`

Admin-only emergency withdrawal from the reserve category while contract is paused.

```
emergency_withdraw(
  caller: Address,
  to: Address,
  amount: i128
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `to` — Recipient address
  - `amount` — Withdrawal amount (must be > 0)

- **Authorization** — `caller` must be admin

- **Preconditions**
  - Contract must be paused
  - Amount must be > 0
  - Reserve balance must be ≥ amount

- **Errors**
  - `Error::InvalidArgument` — Amount is ≤ 0
  - `Error::NotPaused` — Contract is not paused
  - `Error::InsufficientBalance` — Reserve balance insufficient
  - `Error::Unauthorized` — Caller is not admin

- **State Changes** — Decrements reserve category balance

- **Events Emitted**
  - `TREASURY_EMERGENCY_WITHDRAW` legacy event with (caller, to, amount)
  - `ActionExecuted` — `("logging", "action")` with (treasury, emrg_wd, caller, true, timestamp)

### Referral Integration

#### `set_referral_contract`

Register the referral contract authorized to call `distribute_reward` (admin only).

```
set_referral_contract(
  caller: Address,
  referral_contract: Address
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `referral_contract` — Address of the referral contract

- **Authorization** — `caller` must be admin

- **Errors**
  - `Error::Unauthorized` — Caller is not admin

- **Events Emitted**
  - `ActionExecuted` — `("logging", "action")` with (treasury, ref_ctr, caller, true, timestamp)

#### `referral_contract`

Get the currently registered referral contract address.

```
referral_contract(env: Env) -> Option<Address>
```

- **Returns** — Referral contract address or `None` if not set

#### `distribute_reward`

Pay a referral commission from the rewards category (referral contract only).

```
distribute_reward(
  recipient: Address,
  amount: i128
) -> Result<(), Error>
```

- **Parameters**
  - `recipient` — Address receiving the commission
  - `amount` — Commission amount (must be > 0)

- **Authorization** — Direct caller must be the registered referral contract (`require_auth()`)

- **Validation Order** (cheap-first for gas optimization)
  1. `amount > 0`
  2. `amount ≤ rewards category balance`
  3. Direct caller is registered referral contract

- **Errors**
  - `Error::InvalidArgument` — Amount is ≤ 0
  - `Error::InsufficientBalance` — Rewards balance insufficient
  - `Error::Unauthorized` — No referral contract registered or caller is not it

- **State Changes** — Decrements rewards category balance

- **Events Emitted**
  - `CommissionPaid` — `("comm", "paid")` with (recipient, amount, timestamp)
  - `ActionExecuted` — `("logging", "action")` with (treasury, reward, recipient, true, timestamp)

---

## referral-contract

Multi-tier referral graph and commission accrual. Tracks referrer chains, computes commissions at each tier, enforces lifetime caps, and distributes rewards.

### Initialization

#### `initialize`

Initialize the referral contract (required before other calls; admin only).

```
initialize(env: Env, admin: Address)
```

- **Parameters**
  - `admin` — Admin address with governance privileges

- **Initial State**
  - Default max tiers: 1
  - Default reward cap: 0
  - Default tier 1 BPS: 0

- **Events Emitted**
  - `ModuleInitialized` — `("logging", "initialized")` with module name, version, caller, timestamp

### Configuration

#### `set_treasury`

Configure the treasury contract used for reward claims (admin only).

```
set_treasury(
  caller: Address,
  treasury: Address
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `treasury` — Treasury contract address

- **Authorization** — `caller` must be admin

- **Errors**
  - `Error::Unauthorized` — Caller is not admin

- **Events Emitted**
  - `ActionExecuted` — `("logging", "action")` with (referral, treasury, caller, true, timestamp)

#### `get_treasury`

Get the configured treasury contract address.

```
get_treasury(env: Env) -> Result<Address, Error>
```

- **Returns** — Treasury address

- **Errors**
  - `Error::NotFound` — No treasury configured

#### `set_registry`

Configure the registry contract that resolves dependencies (admin only).

```
set_registry(
  caller: Address,
  registry: Address
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `registry` — Registry contract address

- **Authorization** — `caller` must be admin

- **Errors**
  - `Error::Unauthorized` — Caller is not admin

#### `set_tier_config`

Configure tier percentages and lifetime reward cap (admin only).

```
set_tier_config(
  caller: Address,
  tier_bps: Vec<i128>,
  max_tiers: u32,
  reward_cap: i128
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `tier_bps` — Vector of basis point percentages (1 BPS = 0.01%)
  - `max_tiers` — Max referral depth to traverse (1-10)
  - `reward_cap` — Lifetime accrual cap per referrer (0-1_000_000_000_000_000_000)

- **Authorization** — `caller` must be admin

- **Validation**
  - Each tier BPS must be 0-10_000 (0-100%)
  - `max_tiers` must be 1-10
  - `reward_cap` must be 0-1_000_000_000_000_000_000
  - Length of `tier_bps` must equal `max_tiers`

- **Errors**
  - `Error::Unauthorized` — Caller is not admin
  - `Error::InvalidArgument` — Config values out of range or mismatched lengths

- **Events Emitted**
  - `TierConfigSet` — Custom event with full config
  - `PermissionChanged` — `("logging", "permission")` with (referral, tier, caller, true, timestamp)

#### `get_tier_config`

Get the active tier configuration.

```
get_tier_config(env: Env) -> Result<TierConfig, Error>
```

- **Returns** — `TierConfig { tier_bps: Vec<i128>, max_tiers: u32, reward_cap: i128 }`

### Referral Graph Management

#### `set_referrer`

Manually register a referral edge (admin only).

```
set_referrer(
  caller: Address,
  referred_wallet: Address,
  referrer: Address
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `referred_wallet` — Wallet to assign a referrer
  - `referrer` — The referrer address

- **Authorization** — `caller` must be admin

- **Validation**
  - Cannot create self-referral
  - Cannot create cycles in the referral graph

- **Errors**
  - `Error::Unauthorized` — Caller is not admin
  - `Error::InvalidArgument` — Self-referral or would create cycle

- **Events Emitted**
  - `ReferrerSet` — Custom event with (referred_wallet, referrer)
  - `ActionExecuted` — `("logging", "action")` with (referral, referrer, caller, true, timestamp)

#### `get_referrer`

Get the direct referrer for a wallet.

```
get_referrer(env: Env, wallet: Address) -> Option<Address>
```

- **Parameters**
  - `wallet` — Wallet to query

- **Returns** — Direct referrer address or `None` if not registered

#### `register`

Self-register under an existing referrer.

```
register(
  wallet: Address,
  referrer: Address
) -> Result<(), Error>
```

- **Parameters**
  - `wallet` — The wallet registering (must invoke)
  - `referrer` — Existing referrer to join under

- **Authorization** — `wallet` must invoke (`require_auth()`)

- **Validation**
  - Cannot self-refer
  - Cannot register if already registered
  - Referrer must already exist in the graph
  - Cannot create cycles

- **Errors**
  - `Error::InvalidArgument` — Self-referral, already registered, referrer not found, or would create cycle

- **State Changes** — Records referral edge and creates referral record

- **Events Emitted**
  - `ReferralRegistered` — Custom event with (wallet, referrer)
  - `ActionExecuted` — `("logging", "action")` with (referral, register, wallet, true, timestamp)

#### `get_referral_record`

Get the full referral record for a wallet.

```
get_referral_record(env: Env, wallet: Address) -> Option<ReferralRecord>
```

- **Parameters**
  - `wallet` — Wallet to query

- **Returns** — `ReferralRecord { wallet, referrer, commission, tier }` or `None` if not registered

### Commission Accrual

#### `accrue`

Accrue multi-tier referral commissions for a transaction (admin only).

```
accrue(
  caller: Address,
  referred_wallet: Address,
  base_amount: i128
) -> Result<i128, Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `referred_wallet` — The wallet originating the commission
  - `base_amount` — Base transaction amount (must be > 0)

- **Authorization** — `caller` must be admin

- **Algorithm**
  - Traverses the referral chain up to `max_tiers` levels
  - At each tier, calculates commission as `base_amount × tier_bps[tier-1] / 10000`
  - Credits commission to referrer if > 0
  - Applies lifetime `reward_cap` per referrer
  - Pre-caches tier BPS to optimize gas (avoids repeated storage reads)

- **Errors**
  - `Error::Unauthorized` — Caller is not admin
  - `Error::InvalidArgument` — Base amount is ≤ 0
  - `Error::NotFound` — Tier BPS not configured
  - `Error::Overflow` — Commission calculation would overflow

- **Returns** — Total amount credited across all tiers

- **Events Emitted** (per credited referrer)
  - `AccruedReward` — Custom event with (referred_wallet, referrer, tier, amount, accrued_balance, lifetime_accrued)

#### `accrued_balance`

Get the currently claimable accrued balance for a referrer.

```
accrued_balance(env: Env, referrer: Address) -> i128
```

- **Parameters**
  - `referrer` — Referrer to query

- **Returns** — Amount ready to claim (0 if nothing accrued or already claimed)

#### `lifetime_accrued`

Get total lifetime rewards accrued for a referrer (for cap enforcement).

```
lifetime_accrued(env: Env, referrer: Address) -> i128
```

- **Parameters**
  - `referrer` — Referrer to query

- **Returns** — Cumulative lifetime commissions (used to enforce cap)

### Reward Claims

#### `claim_rewards`

Claim accrued referral rewards from treasury.

```
claim_rewards(env: Env, referrer: Address) -> Result<i128, Error>
```

- **Parameters**
  - `referrer` — Referrer claiming their balance

- **Authorization** — `referrer` must invoke (`require_auth()`)

- **Idempotence** — A second claim after a successful payout returns `Ok(0)` and leaves treasury untouched

- **Errors**
  - `Error::NotFound` — Treasury not configured

- **State Changes** — Resets accrued balance to 0

- **Treasury Integration** — Calls `treasury.distribute_reward(referrer, amount)`

- **Events Emitted**
  - `CommissionPaid` — Custom event with (referrer, amount)
  - `ActionExecuted` — `("logging", "action")` with (referral, claim, referrer, true, timestamp)

---

## governance-contract

Multi-signature governance for role management, proposal flow, and protocol parameter tuning.

### Initialization

#### `initialize`

Initialize the governance contract with an admin set and multi-sig threshold (required; admin only).

```
initialize(
  admin: Address,
  threshold: u32,
  admin_set: Vec<Address>
) -> Result<(), Error>
```

- **Parameters**
  - `admin` — Primary admin address (used for backwards-compatible parameter management)
  - `threshold` — Min approvals (M) required to execute proposals; must be ≥ 1 and ≤ `admin_set.len()`
  - `admin_set` — Vector of addresses that can approve proposals; all receive the `Admin` role

- **Validation**
  - `threshold ≥ 1`
  - `admin_set.len() ≥ threshold`

- **Errors**
  - `Error::InvalidArgument` — Threshold or admin set invalid

- **Initial State** — All parameters seeded with defaults:
  - `AidDefaultExpiry`: 604,800 (7 days)
  - `TreasuryWithdrawalLimit`: 100,000,000,000
  - `ReferralTierBps(1)`: 500 (5%)
  - `ReferralTierBps(2)`: 250 (2.5%)
  - `ReferralTierBps(3)`: 100 (1%)
  - `ReferralMaxTiers`: 3
  - `ReferralRewardCap`: 10,000,000,000

- **Events Emitted**
  - `ModuleInitialized` — `("logging", "initialized")` with module name, version, caller, timestamp

### Role Management

#### `grant_role`

Grant a role to a user (admin only).

```
grant_role(
  caller: Address,
  user: Address,
  role: Role
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be an admin
  - `user` — User to grant role
  - `role` — One of: `Admin`, `Upgrader`, `TreasuryManager`, `Pauser`, `ReferralManager`, `OracleSigner`

- **Authorization** — `caller` must hold `Admin` role

- **Errors**
  - `Error::Unauthorized` — Caller is not an admin

- **Events Emitted**
  - `RoleGranted` — Custom event with (caller, user, role_name, timestamp)

#### `revoke_role`

Revoke a role from a user (admin only).

```
revoke_role(
  caller: Address,
  user: Address,
  role: Role
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be an admin
  - `user` — User to revoke role
  - `role` — Role to remove

- **Authorization** — `caller` must hold `Admin` role

- **Errors**
  - `Error::Unauthorized` — Caller is not an admin

- **Events Emitted**
  - `RoleRevoked` — Custom event with (caller, user, role_name, timestamp)

#### `has_role`

Check if a user holds a role.

```
has_role(env: Env, user: Address, role: Role) -> bool
```

- **Parameters**
  - `user` — User to check
  - `role` — Role to verify

- **Returns** — `true` if user holds role, `false` otherwise

### Admin Set Management

#### `set_admin_set`

Update the multi-sig admin set and threshold (admin only).

```
set_admin_set(
  caller: Address,
  new_admin_set: Vec<Address>,
  new_threshold: u32
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be an admin
  - `new_admin_set` — New set of admin addresses
  - `new_threshold` — New approval threshold (M)

- **Authorization** — `caller` must hold `Admin` role

- **Validation**
  - `new_threshold ≥ 1`
  - `new_admin_set.len() ≥ new_threshold`

- **Errors**
  - `Error::Unauthorized` — Caller is not an admin
  - `Error::InvalidArgument` — Threshold or set invalid

#### `get_threshold`

Get the current multi-sig approval threshold.

```
get_threshold(env: Env) -> u32
```

- **Returns** — Current threshold (M)

#### `get_admin_set`

Get the current admin set.

```
get_admin_set(env: Env) -> Vec<Address>
```

- **Returns** — Vector of admin addresses (N)

#### `is_paused`

Check if the contract is currently paused.

```
is_paused(env: Env) -> bool
```

- **Returns** — `true` if paused, `false` otherwise

### Multi-Sig Proposal Flow

#### `propose`

Create a new proposal (admin only).

```
propose(
  caller: Address,
  action: ProposalAction
) -> Result<u64, Error>
```

- **Parameters**
  - `caller` — Must be an admin
  - `action` — One of:
    - `GrantRole(address, role)` — Grant role to address
    - `RevokeRole(address, role)` — Revoke role from address
    - `SetParameter(key, value)` — Update protocol parameter
    - `Pause` — Pause the protocol
    - `Unpause` — Resume the protocol

- **Authorization** — `caller` must hold `Admin` role

- **Returns** — Newly assigned proposal ID

- **Errors**
  - `Error::Unauthorized` — Caller is not an admin
  - `Error::Overflow` — Proposal ID counter overflowed

- **Initial State** — Proposal created with `approval_count = 0` and status `Pending`

- **Events Emitted**
  - `ProposalCreated` — Custom event with (proposal_id, proposer, action_symbol, timestamp)

#### `approve`

Approve a pending proposal (admin only).

```
approve(
  caller: Address,
  proposal_id: u64
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be an admin
  - `proposal_id` — ID of proposal to approve

- **Authorization** — `caller` must hold `Admin` role

- **Idempotence** — Each admin can only approve once; duplicate approvals rejected

- **Errors**
  - `Error::Unauthorized` — Caller is not an admin
  - `Error::ProposalNotFound` — Proposal ID doesn't exist
  - `Error::AlreadyExecuted` — Proposal was already executed
  - `Error::AlreadyApproved` — Caller already approved this proposal
  - `Error::Overflow` — Approval count overflowed

- **State Changes** — Increments proposal `approval_count`

- **Events Emitted**
  - `ProposalApproved` — Custom event with (proposal_id, approver, approval_count, timestamp)

#### `execute`

Execute a proposal once threshold is met (admin only).

```
execute(
  caller: Address,
  proposal_id: u64
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be an admin
  - `proposal_id` — ID of proposal to execute

- **Authorization** — `caller` must hold `Admin` role

- **Preconditions**
  - Proposal must exist and be pending
  - `approval_count ≥ threshold`

- **Errors**
  - `Error::Unauthorized` — Caller is not an admin
  - `Error::ProposalNotFound` — Proposal ID doesn't exist
  - `Error::AlreadyExecuted` — Proposal was already executed
  - `Error::BelowThreshold` — Approval count < threshold
  - `Error::InvalidArgument` — Parameter value out of bounds (for `SetParameter` actions)

- **Action Execution** — Applies the proposal's action (grant/revoke role, set parameter, pause/unpause)

- **State Changes** — Sets proposal status to `Executed`

- **Events Emitted**
  - `ProposalExecuted` — Custom event with (proposal_id, executor, approval_count, timestamp)

#### `get_proposal`

Get a proposal by ID.

```
get_proposal(env: Env, proposal_id: u64) -> Result<Proposal, Error>
```

- **Parameters**
  - `proposal_id` — ID to fetch

- **Returns** — `Proposal { id, proposer, action, approval_count, status, created_at }`

- **Errors**
  - `Error::ProposalNotFound` — Proposal doesn't exist

### Parameter Management

#### `set_param`

Update a protocol parameter (admin only).

```
set_param(
  caller: Address,
  key: ParameterKey,
  value: i128
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the primary admin
  - `key` — Parameter key (see list below)
  - `value` — New value (must be within documented bounds)

- **Authorization** — `caller` must be primary admin

- **Parameter Keys & Bounds**
  - `AidDefaultExpiry`: 60-31,536,000 seconds
  - `TreasuryWithdrawalLimit`: 0-1,000,000,000,000,000,000
  - `ReferralTierBps(tier)`: 0-10,000 (0-100% in basis points)
  - `ReferralMaxTiers`: 1-10
  - `ReferralRewardCap`: 0-1,000,000,000,000,000,000

- **Errors**
  - `Error::Unauthorized` — Caller is not primary admin
  - `Error::InvalidArgument` — Value outside bounds or tier invalid

- **Events Emitted**
  - `ParameterChanged` — Custom event with (key, value)
  - `ActionExecuted` — `("logging", "action")` with (gov, set_param, caller, true, timestamp)

#### `get_param`

Read a protocol parameter.

```
get_param(env: Env, key: ParameterKey) -> Result<i128, Error>
```

- **Parameters**
  - `key` — Parameter to read

- **Returns** — Parameter value

- **Errors**
  - `Error::NotFound` — Parameter not set

#### `get_bounds`

Get documented min/max bounds for a parameter.

```
get_bounds(env: Env, key: ParameterKey) -> Result<ParameterBounds, Error>
```

- **Parameters**
  - `key` — Parameter key

- **Returns** — `ParameterBounds { min: i128, max: i128 }`

- **Errors**
  - `Error::InvalidArgument` — Parameter key invalid

#### Convenience Getters

```
aid_default_expiry(env: Env) -> Result<i128, Error>
treasury_withdrawal_limit(env: Env) -> Result<i128, Error>
referral_tier_bps(env: Env, tier: u32) -> Result<i128, Error>
referral_max_tiers(env: Env) -> Result<i128, Error>
referral_reward_cap(env: Env) -> Result<i128, Error>
```

Each returns the current value for the corresponding parameter.

---

## registry-contract

Contract and metadata registry. Tracks contract versions, manages mutable/immutable metadata entries, enables contract discovery.

### Initialization

#### `initialize`

Initialize the registry contract (required before other calls; admin only).

```
initialize(env: Env, admin: Address)
```

- **Parameters**
  - `admin` — Admin address with governance privileges

- **Events Emitted**
  - `ModuleInitialized` — `("logging", "initialized")` with module name, version, caller, timestamp

### Contract Registration

#### `set_contract`

Register or update a contract address and version (admin only).

```
set_contract(
  caller: Address,
  name: Symbol,
  address: Address,
  version: u32
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `name` — Logical contract name (e.g., "aid", "treasury")
  - `address` — Contract address to register
  - `version` — Version number (typically incremented on upgrades)

- **Authorization** — `caller` must be admin

- **Gas Optimization** — Version history Vec only deserialized when genuinely new version registered; common case (same version) skips expensive Vec read + linear scan

- **Errors**
  - `Error::Unauthorized` — Caller is not admin

- **State Changes** — Updates contract mapping and appends to version history if new version

- **Events Emitted**
  - `ActionExecuted` — `("logging", "action")` with (registry, set_ctr, caller, true, timestamp)

#### `get_contract`

Resolve the latest registered address and version for a contract name.

```
get_contract(env: Env, name: Symbol) -> Result<(Address, u32), Error>
```

- **Parameters**
  - `name` — Contract name to look up

- **Returns** — `(Address, version_number)` tuple

- **Errors**
  - `Error::NotFound` — Contract name not registered

#### `get_version_history`

Get the complete version history for a contract name.

```
get_version_history(env: Env, name: Symbol) -> Result<Vec<u32>, Error>
```

- **Parameters**
  - `name` — Contract name to query

- **Returns** — Vector of all registered versions in chronological order

- **Errors**
  - `Error::NotFound` — Contract name not registered

### Metadata Registry

#### `set_metadata`

Register or update metadata for an identifier (admin only).

```
set_metadata(
  caller: Address,
  name: Symbol,
  uri: Bytes,
  hash: Bytes,
  immutable: bool,
  schema_version: u32
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `name` — Identifier for the metadata (e.g., "treasury_config", "oracle_feed")
  - `uri` — URI pointing to metadata content (IPFS, HTTPS, etc.)
  - `hash` — SHA-256 hash (32 bytes) of the metadata for verification
  - `immutable` — If `true`, this entry cannot be updated after creation
  - `schema_version` — Version of the metadata schema

- **Authorization** — `caller` must be admin

- **Validation**
  - Hash must be exactly 32 bytes (SHA-256)
  - URI must not be empty
  - Cannot update an immutable entry

- **Errors**
  - `Error::Unauthorized` — Caller is not admin
  - `Error::ImmutableEntry` — Entry exists and is marked immutable
  - `Error::InvalidHash` — Hash is not 32 bytes
  - `Error::InvalidArgument` — URI is empty

- **State Changes** — Creates or updates metadata entry with current timestamp

#### `get_metadata`

Retrieve full metadata entry for an identifier.

```
get_metadata(env: Env, name: Symbol) -> Result<MetadataEntry, Error>
```

- **Parameters**
  - `name` — Metadata identifier

- **Returns** — `MetadataEntry { uri, hash, immutable, updated_at, schema_version }`

- **Errors**
  - `Error::MetadataNotFound` — Metadata not registered

#### `get_metadata_hash`

Get the metadata hash for an identifier (for verification).

```
get_metadata_hash(env: Env, name: Symbol) -> Result<Bytes, Error>
```

- **Parameters**
  - `name` — Metadata identifier

- **Returns** — 32-byte SHA-256 hash

- **Errors**
  - `Error::MetadataNotFound` — Metadata not registered

#### `is_immutable`

Check if a metadata entry is immutable.

```
is_immutable(env: Env, name: Symbol) -> Result<bool, Error>
```

- **Parameters**
  - `name` — Metadata identifier

- **Returns** — `true` if entry is immutable, `false` otherwise

- **Errors**
  - `Error::MetadataNotFound` — Metadata not registered

### Combined Registry Entry

#### `get_registry_entry`

Get the full registry entry (contract + metadata) for an identifier.

```
get_registry_entry(env: Env, name: Symbol) -> Result<RegistryEntry, Error>
```

- **Parameters**
  - `name` — Identifier to query

- **Returns** — `RegistryEntry { contract: ContractRegistration, metadata: MetadataEntry }`
  - `ContractRegistration { address, version }`
  - `MetadataEntry { uri, hash, immutable, updated_at, schema_version }`

- **Errors**
  - `Error::NotFound` — Contract not registered
  - `Error::MetadataNotFound` — Metadata not registered

### Listing

#### `list_names`

List all registered contract names.

```
list_names(env: Env) -> Result<Vec<Symbol>, Error>
```

- **Returns** — Vector of all contract names that have been registered

#### `list_metadata_entries`

List all metadata entries.

```
list_metadata_entries(env: Env) -> Result<Map<Symbol, MetadataEntry>, Error>
```

- **Returns** — Map of identifier → `MetadataEntry` for all registered metadata

---

## payments-contract

Example payment gateway demonstrating token deposits, pull-based withdrawals with fee handling, escrow, and batch payouts.

### Initialization

#### `initialize`

Initialize the payment gateway (required before other calls; admin only).

```
initialize(
  admin: Address,
  token: Address,
  fee_rate_bps: i128,
  fee_recipient: Address
) -> Result<(), Error>
```

- **Parameters**
  - `admin` — Admin address with governance privileges
  - `token` — Token address for all deposits/withdrawals
  - `fee_rate_bps` — Fee rate in basis points (0-10,000; 0-100%)
  - `fee_recipient` — Address receiving collected fees

- **Validation**
  - `fee_rate_bps` must be 0-10,000

- **Errors**
  - `Error::PaymentInvalidFeeRate` — Fee rate outside valid range

- **Initial State** — Total deposits set to 0

- **Events Emitted**
  - `ModuleInitialized` — `("logging", "initialized")` with module name, version, caller, timestamp

### Configuration (Admin Only)

#### `set_fee_rate`

Update the fee rate (admin only).

```
set_fee_rate(
  caller: Address,
  new_rate_bps: i128
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `new_rate_bps` — New fee rate (0-10,000 basis points)

- **Authorization** — `caller` must be admin

- **Validation**
  - `new_rate_bps` must be 0-10,000

- **Errors**
  - `Error::Unauthorized` — Caller is not admin
  - `Error::PaymentInvalidFeeRate` — Fee rate outside valid range

- **Events Emitted**
  - `ActionExecuted` — `("logging", "action")` with (pay_gw, fee_set, caller, true, timestamp)

#### `fee_rate`

Get the current fee rate in basis points.

```
fee_rate(env: Env) -> i128
```

- **Returns** — Fee rate (0-10,000)

#### `fee_recipient`

Get the configured fee recipient address.

```
fee_recipient(env: Env) -> Option<Address>
```

- **Returns** — Fee recipient address or `None`

#### `get_token`

Get the configured token address.

```
get_token(env: Env) -> Option<Address>
```

- **Returns** — Token address or `None`

### Deposits

#### `deposit`

Deposit tokens into the contract.

```
deposit(env: Env, from: Address, amount: i128) -> Result<(), Error>
```

- **Parameters**
  - `from` — Depositor address (must invoke with `require_auth()`)
  - `amount` — Deposit amount (must be > 0)

- **Authorization** — `from` must invoke (`require_auth()`)

- **Preconditions**
  - `from` must have pre-authorized the contract to transfer tokens
  - Amount must be > 0

- **Errors**
  - `Error::PaymentInvalidAmount` — Amount is ≤ 0
  - `Error::NotFound` — Token not configured
  - `Error::Overflow` — Balance or total would overflow

- **Token Transfer** — Transfers `amount` from depositor to contract

- **State Changes** — Credits amount to depositor's internal balance; updates total deposits

- **Events Emitted**
  - `TreasuryDeposit` — Custom event with (category, depositor, amount, new_balance)

#### `balance_of`

Get the internal balance for a depositor.

```
balance_of(env: Env, who: Address) -> i128
```

- **Parameters**
  - `who` — Address to query

- **Returns** — Current balance (0 if never deposited)

#### `total_deposits`

Get total deposits across all users.

```
total_deposits(env: Env) -> i128
```

- **Returns** — Sum of all deposited amounts

### Withdrawals (Pull-Based with Fee)

#### `withdraw`

Withdraw the caller's full balance, deducting configured fee.

```
withdraw(env: Env, who: Address) -> Result<i128, Error>
```

- **Parameters**
  - `who` — Withdrawing address (must invoke with `require_auth()`)

- **Authorization** — `who` must invoke (`require_auth()`)

- **Algorithm**
  - Calculates fee: `deposited × fee_rate / 10,000`
  - Calculates net: `deposited - fee`
  - Transfers fee to `fee_recipient`
  - Transfers net to `who`

- **Errors**
  - `Error::PaymentInsufficientBalance` — No deposited balance
  - `Error::NotFound` — Token or fee recipient not configured

- **State Changes** — Clears internal balance; decrements total deposits

- **Returns** — Net amount received (after fee)

- **Events Emitted**
  - `TreasuryWithdraw` — Custom event with (category, who, amount, net)

#### `withdraw_amount`

Withdraw a specific amount from the caller's balance, deducting fee.

```
withdraw_amount(
  env: Env,
  who: Address,
  amount: i128
) -> Result<i128, Error>
```

- **Parameters**
  - `who` — Withdrawing address (must invoke with `require_auth()`)
  - `amount` — Amount to withdraw (must be > 0)

- **Authorization** — `who` must invoke (`require_auth()`)

- **Validation**
  - `amount > 0`
  - `amount ≤ balance_of(who)`

- **Algorithm**
  - Calculates fee: `amount × fee_rate / 10,000`
  - Calculates net: `amount - fee`
  - Transfers fee to `fee_recipient`
  - Transfers net to `who`

- **Errors**
  - `Error::PaymentInvalidAmount` — Amount is ≤ 0
  - `Error::PaymentInsufficientBalance` — Amount exceeds balance
  - `Error::NotFound` — Token or fee recipient not configured

- **State Changes** — Decrements internal balance; decrements total deposits

- **Returns** — Net amount received (after fee)

- **Events Emitted**
  - `TreasuryWithdraw` — Custom event with (category, who, amount, net)

### Escrow

#### `create_escrow_entry`

Create an escrow deposit from depositor to beneficiary.

```
create_escrow_entry(
  depositor: Address,
  beneficiary: Address,
  amount: i128,
  expiry_ledger: u32
) -> Result<u64, Error>
```

- **Parameters**
  - `depositor` — Funding address (must invoke with `require_auth()`)
  - `beneficiary` — Recipient address
  - `amount` — Escrow amount (must be > 0)
  - `expiry_ledger` — Ledger sequence after which escrow can be refunded

- **Authorization** — `depositor` must invoke (`require_auth()`)

- **Returns** — Newly created escrow ID

- **Errors**
  - `Error::NotFound` — Token not configured
  - `Error::PaymentInvalidAmount` — Amount is ≤ 0

- **State Changes** — Transfers amount from depositor to contract; records escrow entry

#### `release_escrow_entry`

Release an escrow deposit to the beneficiary (admin only).

```
release_escrow_entry(
  caller: Address,
  escrow_id: u64
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `escrow_id` — ID of escrow to release

- **Authorization** — `caller` must be admin

- **Errors**
  - `Error::Unauthorized` — Caller is not admin
  - `Error::NotFound` — Escrow ID doesn't exist or already released
  - Token transfer errors

- **State Changes** — Transfers escrowed amount to beneficiary; marks escrow as released

#### `refund_escrow_entry`

Refund an escrow deposit back to the depositor (admin only).

```
refund_escrow_entry(
  caller: Address,
  escrow_id: u64
) -> Result<(), Error>
```

- **Parameters**
  - `caller` — Must be the current admin
  - `escrow_id` — ID of escrow to refund

- **Authorization** — `caller` must be admin

- **Errors**
  - `Error::Unauthorized` — Caller is not admin
  - `Error::NotFound` — Escrow ID doesn't exist or already released
  - Token transfer errors

- **State Changes** — Transfers escrowed amount back to depositor; marks escrow as refunded

#### `get_escrow_entry`

Read an escrow record.

```
get_escrow_entry(env: Env, escrow_id: u64) -> Option<EscrowRecord>
```

- **Parameters**
  - `escrow_id` — ID to query

- **Returns** — `EscrowRecord { depositor, beneficiary, amount, expiry_ledger, status }` or `None` if not found

### Batch Payouts

#### `batch_payout`

Distribute tokens from contract to multiple recipients atomically (admin only).

```
batch_payout(
  caller: Address,
  recipients: Vec<(Address, i128)>
) -> BatchResult
```

- **Parameters**
  - `caller` — Must be the current admin
  - `recipients` — Vector of `(recipient_address, amount)` tuples

- **Authorization** — `caller` must be admin

- **Algorithm** — Uses shared `multi_transfer_all` batch executor in atomic mode
  - On any transfer failure, entire batch reverts
  - All-or-nothing semantics

- **Returns** — `BatchResult { total, succeeded, failed, reverted }`

- **Note** — This is an example of how to use the shared batch payment utilities; production deployments may require additional validation or non-atomic mode for resilience.

---

## access-control

Standalone Role-Based Access Control (RBAC) with hierarchical roles, multi-admin management, and cycle detection.

### Initialization

#### `initialize`

Initialize the access control contract with a super-admin (required; called once).

```
initialize(env: Env, super_admin: Address)
```

- **Parameters**
  - `super_admin` — Super-admin address (root of hierarchy)

- **Initial State**
  - Super-admin registered as admin
  - `super_admin` role created and granted to super-admin
  - Super-admin cannot be removed via normal path

### Admin Management

#### `is_admin`

Check if an address is a registered admin.

```
is_admin(env: Env, who: Address) -> bool
```

- **Parameters**
  - `who` — Address to check

- **Returns** — `true` if admin, `false` otherwise

#### `super_admin`

Get the super-admin address set at initialization.

```
super_admin(env: Env) -> Address
```

- **Returns** — Super-admin address

#### `add_admin`

Add a new admin (admin-only; idempotent).

```
add_admin(
  caller: Address,
  new_admin: Address
) -> Result<(), AccessControlError>
```

- **Parameters**
  - `caller` — Must be an existing admin
  - `new_admin` — Address to grant admin status

- **Authorization** — `caller` must be admin and invoke (`require_auth()`)

- **Idempotence** — If address already admin, succeeds without error

- **Errors**
  - `AccessControlError::NotAdmin` — Caller is not an admin

- **Events Emitted**
  - `EV_ADMIN_ADDED` — Custom event with (caller, new_admin)

#### `remove_admin`

Remove an admin (admin-only; idempotent; cannot remove super-admin).

```
remove_admin(
  caller: Address,
  target: Address
) -> Result<(), AccessControlError>
```

- **Parameters**
  - `caller` — Must be an existing admin
  - `target` — Address to revoke admin status

- **Authorization** — `caller` must be admin and invoke (`require_auth()`)

- **Preconditions**
  - Cannot remove the super-admin

- **Idempotence** — If address not admin, succeeds without error

- **Errors**
  - `AccessControlError::NotAdmin` — Caller is not an admin
  - `AccessControlError::CannotRemoveSuperAdmin` — Target is the super-admin

- **Events Emitted**
  - `EV_ADMIN_REMOVED` — Custom event with (caller, target)

#### `get_all_admins`

Get the list of all current admins.

```
get_all_admins(env: Env) -> Vec<Address>
```

- **Returns** — Vector of admin addresses

### Role Creation

#### `create_role`

Create a new role (admin-only).

```
create_role(
  caller: Address,
  role: Symbol
) -> Result<(), AccessControlError>
```

- **Parameters**
  - `caller` — Must be an admin
  - `role` — New role symbol (max 9 chars for `symbol_short!`)

- **Authorization** — `caller` must be admin and invoke (`require_auth()`)

- **Errors**
  - `AccessControlError::NotAdmin` — Caller is not an admin
  - `AccessControlError::RoleAlreadyExists` — Role already created
  - `AccessControlError::InvalidRole` — Role symbol invalid (empty)

- **Events Emitted**
  - `EV_ROLE_CREATED` — Custom event with (role, caller)

#### `role_exists_check`

Check if a role has been registered.

```
role_exists_check(env: Env, role: Symbol) -> bool
```

- **Parameters**
  - `role` — Role to check

- **Returns** — `true` if registered, `false` otherwise

#### `get_all_roles`

Get the list of all registered roles.

```
get_all_roles(env: Env) -> Vec<Symbol>
```

- **Returns** — Vector of all role symbols (includes `super_admin`)

### Role Hierarchy

#### `set_role_parent`

Set a parent role (establishes hierarchy; admin-only).

```
set_role_parent(
  caller: Address,
  role: Symbol,
  parent: Symbol
) -> Result<(), AccessControlError>
```

- **Parameters**
  - `caller` — Must be an admin
  - `role` — Child role
  - `parent` — Parent role (holder of parent satisfies child checks)

- **Authorization** — `caller` must be admin and invoke (`require_auth()`)

- **Semantics** — A holder of `parent` automatically satisfies any `has_role` check for `role` (and all ancestors of `role`)

- **Validation**
  - Both roles must exist
  - Cannot self-reference (`role == parent`)
  - Cannot create cycles (parent cannot have role as ancestor)

- **Errors**
  - `AccessControlError::NotAdmin` — Caller is not an admin
  - `AccessControlError::RoleNotFound` — Role or parent doesn't exist
  - `AccessControlError::SelfReference` — `role == parent`
  - `AccessControlError::CycleDetected` — Would create cycle

- **Events Emitted**
  - `EV_ROLE_PARENT_SET` — Custom event with (role, parent, caller)

#### `get_role_parent`

Get the direct parent of a role.

```
get_role_parent(env: Env, role: Symbol) -> Option<Symbol>
```

- **Parameters**
  - `role` — Role to query

- **Returns** — Direct parent role or `None` if not set

#### `get_role_ancestors`

Get the full ancestor chain for a role (ordered parent → grandparent → ...).

```
get_role_ancestors(env: Env, role: Symbol) -> Vec<Symbol>
```

- **Parameters**
  - `role` — Role to query

- **Returns** — Vector of ancestor roles (excludes the role itself), ordered from immediate parent upward

### Grant / Revoke

#### `grant_role`

Grant a role to a user (admin-only).

```
grant_role(
  caller: Address,
  role: Symbol,
  user: Address
) -> Result<(), AccessControlError>
```

- **Parameters**
  - `caller` — Must be an admin
  - `role` — Role to grant
  - `user` — User to receive role

- **Authorization** — `caller` must be admin and invoke (`require_auth()`)

- **Errors**
  - `AccessControlError::NotAdmin` — Caller is not an admin
  - `AccessControlError::RoleNotFound` — Role not registered

- **Events Emitted**
  - `EV_ROLE_GRANTED` — Custom event with (role, user, caller)

#### `revoke_role`

Revoke a role from a user (admin-only; idempotent).

```
revoke_role(
  caller: Address,
  role: Symbol,
  user: Address
) -> Result<(), AccessControlError>
```

- **Parameters**
  - `caller` — Must be an admin
  - `role` — Role to revoke
  - `user` — User to revoke from

- **Authorization** — `caller` must be admin and invoke (`require_auth()`)

- **Idempotence** — Succeeds even if user didn't hold the role

- **Errors**
  - `AccessControlError::NotAdmin` — Caller is not an admin
  - `AccessControlError::RoleNotFound` — Role not registered

- **Events Emitted**
  - `EV_ROLE_REVOKED` — Custom event with (role, user, caller)

#### `has_role`

Check if a user holds a role (directly or via hierarchy).

```
has_role(env: Env, role: Symbol, user: Address) -> bool
```

- **Parameters**
  - `role` — Role to check
  - `user` — User to verify

- **Returns** — `true` if user holds role directly or via any ancestor

- **Algorithm**
  - Checks direct membership in role
  - If not direct member, walks up role hierarchy
  - Returns `true` if any ancestor holds the user

### Read Helpers

#### `get_role_members`

Get all direct members of a role.

```
get_role_members(env: Env, role: Symbol) -> Vec<Address>
```

- **Parameters**
  - `role` — Role to query

- **Returns** — Vector of addresses that directly hold the role (not including indirect members via hierarchy)

---

## upgradeability

Contract upgrade registry and coordinator with migration hooks, proposal flow, and audit trail.

### Initialization

#### `initialize`

Initialize the upgrade registry (required before other calls; admin only).

```
initialize(env: Env, admin: Address) -> Result<(), UpgradeError>
```

- **Parameters**
  - `admin` — Admin address with governance privileges

- **Initial State**
  - Admin granted `Admin` and `Upgrader` roles
  - Registry initialized (empty)

- **Events Emitted**
  - `ModuleInitialized` — `("logging", "initialized")` with module name, version, caller, timestamp

### Contract Registration

#### `register_contract`

Register a contract for upgrade management (admin only).

```
register_contract(
  caller: Address,
  contract_id: Address,
  name: Symbol,
  version: u32,
  wasm_hash: BytesN<32>
) -> Result<(), UpgradeError>
```

- **Parameters**
  - `caller` — Must hold `Admin` role
  - `contract_id` — On-chain address of contract to register
  - `name` — Logical name (e.g., `symbol_short!("aid")`)
  - `version` — Initial version (must be ≥ 1)
  - `wasm_hash` — WASM hash of initial deployment (32 bytes)

- **Authorization** — `caller` must hold `Admin` role

- **Validation**
  - Version must be ≥ 1
  - Name must not already be registered
  - WASM hash must not be empty

- **Errors**
  - `UpgradeError::NotUpgrader` — Caller doesn't hold Admin role
  - `UpgradeError::InvalidWasmHash` — Version < 1 or hash invalid
  - `UpgradeError::ContractAlreadyRegistered` — Name already registered

- **Events Emitted**
  - `ContractRegistered` — Custom event with (contract_id, name, version, wasm_hash, timestamp)

#### `get_registry_entry`

Get the registry entry for a contract by ID.

```
get_registry_entry(env: Env, contract_id: Address) -> Result<RegistryEntry, UpgradeError>
```

- **Parameters**
  - `contract_id` — Contract address to query

- **Returns** — `RegistryEntry { contract_id, name, current: VersionInfo, migration_hook }`
  - `VersionInfo { version, wasm_hash, deployed_at, description }`

- **Errors**
  - `UpgradeError::ContractNotRegistered` — Contract not registered

#### `get_registry_entry_by_name`

Get the registry entry by logical name.

```
get_registry_entry_by_name(env: Env, name: Symbol) -> Result<RegistryEntry, UpgradeError>
```

- **Parameters**
  - `name` — Logical contract name

- **Returns** — `RegistryEntry` for the named contract

- **Errors**
  - `UpgradeError::ContractNotRegistered` — Name not registered

#### `get_registered_count`

Get the total number of registered contracts.

```
get_registered_count(env: Env) -> u64
```

- **Returns** — Count of registered contracts

#### `get_version`

Get the current version number for a contract.

```
get_version(env: Env, contract_id: Address) -> Result<u32, UpgradeError>
```

- **Parameters**
  - `contract_id` — Contract address

- **Returns** — Current version number

- **Errors**
  - `UpgradeError::ContractNotRegistered` — Contract not registered

#### `get_wasm_hash`

Get the current WASM hash for a contract.

```
get_wasm_hash(env: Env, contract_id: Address) -> Result<BytesN<32>, UpgradeError>
```

- **Parameters**
  - `contract_id` — Contract address

- **Returns** — Current WASM hash (32 bytes)

- **Errors**
  - `UpgradeError::ContractNotRegistered` — Contract not registered

#### `is_registered`

Check if a contract is registered.

```
is_registered(env: Env, contract_id: Address) -> bool
```

- **Parameters**
  - `contract_id` — Contract address

- **Returns** — `true` if registered, `false` otherwise

### Migration Hooks

#### `set_migration_hook`

Register a migration hook contract (admin only).

```
set_migration_hook(
  caller: Address,
  contract_id: Address,
  hook_addr: Address
) -> Result<(), UpgradeError>
```

- **Parameters**
  - `caller` — Must hold `Admin` role
  - `contract_id` — Target contract
  - `hook_addr` — Hook contract address

- **Authorization** — `caller` must hold `Admin` role

- **Hook Interface** — Hook contract must implement:
  - `pre_upgrade(env, old_version, new_version) -> bool` — runs before WASM update; return `false` to abort
  - `post_upgrade(env, old_version, new_version)` — runs after WASM update; for state migrations

- **Errors**
  - `UpgradeError::NotUpgrader` — Caller doesn't hold Admin role
  - `UpgradeError::ContractNotRegistered` — Target contract not registered

- **Events Emitted**
  - `MigrationHookSet` — Custom event with (contract_id, hook_addr, timestamp)

#### `get_migration_hook`

Get the migration hook address for a contract.

```
get_migration_hook(env: Env, contract_id: Address) -> Option<Address>
```

- **Parameters**
  - `contract_id` — Contract address

- **Returns** — Hook contract address or `None` if not set

### Upgrade Proposals

#### `propose_upgrade`

Create an upgrade proposal (upgrader only).

```
propose_upgrade(
  caller: Address,
  contract_id: Address,
  new_wasm_hash: BytesN<32>,
  new_version: u32,
  note: String
) -> Result<u64, UpgradeError>
```

- **Parameters**
  - `caller` — Must hold `Upgrader` role
  - `contract_id` — Target contract
  - `new_wasm_hash` — New WASM hash (32 bytes)
  - `new_version` — New version (must be > current)
  - `note` — Optional migration note

- **Authorization** — `caller` must hold `Upgrader` role

- **Validation**
  - `new_version > current.version`
  - `new_wasm_hash != current.wasm_hash`
  - No pending upgrade already exists

- **Errors**
  - `UpgradeError::NotUpgrader` — Caller doesn't hold Upgrader role
  - `UpgradeError::ContractNotRegistered` — Contract not registered
  - `UpgradeError::NoChangeDetected` — Version or hash unchanged
  - `UpgradeError::AlreadyPending` — Upgrade already pending

- **Returns** — Newly created proposal ID

- **Events Emitted**
  - `UpgradeProposed` — Custom event with (proposal_id, contract_id, new_version, proposer, timestamp)

#### `execute_upgrade`

Execute an upgrade proposal (upgrader only).

```
execute_upgrade(
  caller: Address,
  proposal_id: u64
) -> Result<(), UpgradeError>
```

- **Parameters**
  - `caller` — Must hold `Upgrader` role
  - `proposal_id` — ID of proposal to execute

- **Authorization** — `caller` must hold `Upgrader` role

- **Algorithm**
  1. Validate proposal exists and is pending
  2. Call pre-upgrade migration hook (if present)
  3. Update registry with new version/hash
  4. Call post-upgrade migration hook (if present)
  5. Mark proposal executed
  6. Record in upgrade history
  7. Remove pending status

- **Errors**
  - `UpgradeError::NotUpgrader` — Caller doesn't hold Upgrader role
  - `UpgradeError::ProposalNotFound` — Proposal doesn't exist
  - `UpgradeError::AlreadyExecuted` — Proposal already executed
  - `UpgradeError::ContractNotRegistered` — Target contract not found
  - `UpgradeError::MigrationHookFailed` — Hook call failed or returned false

- **Events Emitted**
  - `UpgradeExecuted` — Custom event with (proposal_id, executor, new_version, timestamp)

#### `get_proposal`

Get a proposal by ID.

```
get_proposal(env: Env, proposal_id: u64) -> Result<UpgradeProposal, UpgradeError>
```

- **Parameters**
  - `proposal_id` — Proposal ID

- **Returns** — `UpgradeProposal { id, contract_id, new_wasm_hash, new_version, note, proposer, executed, created_at, executed_at }`

- **Errors**
  - `UpgradeError::ProposalNotFound` — Proposal doesn't exist

#### `get_pending_proposal`

Get the pending proposal ID for a contract.

```
get_pending_proposal(env: Env, contract_id: Address) -> Option<u64>
```

- **Parameters**
  - `contract_id` — Contract address

- **Returns** — Pending proposal ID or `None` if no upgrade pending

#### `get_upgrade_status`

Get the upgrade status for a contract.

```
get_upgrade_status(env: Env, contract_id: Address) -> Result<UpgradeStatus, UpgradeError>
```

- **Parameters**
  - `contract_id` — Contract address

- **Returns** — One of:
  - `UpgradeStatus::Current` — No upgrade pending
  - `UpgradeStatus::Pending(proposal_id)` — Upgrade proposal pending
  - `UpgradeStatus::Completed` — Upgrade was recently executed

- **Errors**
  - `UpgradeError::ContractNotRegistered` — Contract not registered

#### `cancel_proposal`

Cancel a pending upgrade proposal (proposer or admin).

```
cancel_proposal(
  caller: Address,
  proposal_id: u64
) -> Result<(), UpgradeError>
```

- **Parameters**
  - `caller` — Must be proposer or admin
  - `proposal_id` — ID of proposal to cancel

- **Authorization** — `caller` must be proposer or admin

- **Errors**
  - `UpgradeError::ProposalNotFound` — Proposal doesn't exist
  - `UpgradeError::AlreadyExecuted` — Cannot cancel executed proposal
  - `UpgradeError::NotUpgrader` — Caller is not proposer or admin

- **State Changes** — Removes pending status; proposal remains in history

### Upgrade Authorization

#### `verify_upgrade_authorization`

Verify that an upgrade is authorized (called by target contract).

```
verify_upgrade_authorization(
  caller: Address,
  contract_id: Address,
  wasm_hash: BytesN<32>
) -> Result<(), UpgradeError>
```

- **Parameters**
  - `caller` — Caller initiating the upgrade (must be upgrader)
  - `contract_id` — Target contract
  - `wasm_hash` — WASM hash to verify

- **Authorization** — `caller` must hold `Upgrader` role

- **Validation**
  - Contract must be registered
  - A proposal must exist for this contract
  - Proposal's WASM hash must match the provided hash

- **Errors**
  - `UpgradeError::NotUpgrader` — Caller not upgrader
  - `UpgradeError::ContractNotRegistered` — Contract not registered
  - `UpgradeError::ProposalNotFound` — No pending proposal
  - `UpgradeError::NoChangeDetected` — WASM hash mismatch

### Upgrade History

#### `get_upgrade_history`

Get the upgrade history for a contract.

```
get_upgrade_history(env: Env, contract_id: Address) -> Result<Vec<UpgradeRecord>, UpgradeError>
```

- **Parameters**
  - `contract_id` — Contract address

- **Returns** — Vector of `UpgradeRecord { contract_id, old_version, new_version, old_wasm_hash, new_wasm_hash, executor, executed_at, note }`

- **Errors**
  - `UpgradeError::ContractNotRegistered` — Contract not registered

### Utility

#### `can_upgrade`

Check if a caller can propose/execute upgrades.

```
can_upgrade(env: Env, caller: Address) -> bool
```

- **Parameters**
  - `caller` — Address to check

- **Returns** — `true` if caller holds `Upgrader` role, `false` otherwise

---

## nft-marketplace

Comprehensive NFT marketplace with fixed-price listings, English/Dutch auctions, offers, royalty enforcement, and multi-currency support.

### Initialization

#### `initialize`

Initialize the NFT marketplace (required before other calls; admin only).

```
initialize(
  admin: Address,
  platform_fee_bps: i128,
  fee_recipient: Address,
  bid_increment_bps: i128,
  auto_extension_seconds: u64
) -> Result<(), MarketError>
```

- **Parameters**
  - `admin` — Admin address with governance privileges
  - `platform_fee_bps` — Platform fee in basis points (0-1,000; 0-10%)
  - `fee_recipient` — Address receiving platform fees
  - `bid_increment_bps` — Min bid increment for auctions (0-5,000; 0-50%)
  - `auto_extension_seconds` — Auto-extend window for English auctions (≤ 3,600 sec)

- **Validation**
  - All basis point values must be within range
  - Auto-extension must not exceed 1 hour

- **Errors**
  - `MarketError::InvalidArgument` — Parameters out of range

- **Initial State** — Empty collections, listings, auctions; no currencies whitelisted

- **Events Emitted**
  - `ModuleInitialized` — `("logging", "initialized")` with module, version, caller, timestamp

### Configuration (Admin Only)

#### `set_platform_fee`

Update the platform fee rate (admin only).

```
set_platform_fee(caller: Address, new_fee_bps: i128) -> Result<(), MarketError>
```

- **Authorization** — `caller` must be admin

- **Validation** — Fee must be 0-1,000 basis points

- **Errors**
  - `MarketError::Unauthorized` — Caller not admin
  - `MarketError::InvalidArgument` — Fee out of range

#### `set_fee_recipient`

Update the fee recipient address (admin only).

```
set_fee_recipient(caller: Address, new_recipient: Address) -> Result<(), MarketError>
```

- **Authorization** — `caller` must be admin

#### `set_bid_increment`

Update the minimum bid increment for auctions (admin only).

```
set_bid_increment(caller: Address, bps: i128) -> Result<(), MarketError>
```

- **Authorization** — `caller` must be admin

- **Validation** — Increment must be 0-5,000 basis points

#### `set_auto_extension`

Update the auto-extension window for English auctions (admin only).

```
set_auto_extension(caller: Address, seconds: u64) -> Result<(), MarketError>
```

- **Authorization** — `caller` must be admin

- **Validation** — Window must be ≤ 3,600 seconds (1 hour)

#### `set_currency`

Whitelist or delist a payment currency (admin only).

```
set_currency(
  caller: Address,
  currency: Address,
  whitelisted: bool
) -> Result<(), MarketError>
```

- **Authorization** — `caller` must be admin

#### `set_oracle`

Configure price oracle address (admin only).

```
set_oracle(caller: Address, oracle: Address) -> Result<(), MarketError>
```

- **Authorization** — `caller` must be admin

#### Configuration Getters

```
platform_fee_bps(env: Env) -> i128
fee_recipient_addr(env: Env) -> Address
bid_increment_bps(env: Env) -> i128
auto_extension_seconds(env: Env) -> u64
is_currency_whitelisted(env: Env, currency: Address) -> bool
oracle_address(env: Env) -> Option<Address>
```

### Collection Management

#### `register_collection`

Register an NFT collection with royalty config (admin only).

```
register_collection(
  caller: Address,
  collection: Address,
  name: Symbol,
  ipfs_uri: Bytes,
  metadata_hash: Bytes,
  standard: TokenStandard,
  royalty_recipients: Vec<RoyaltyRecipient>
) -> Result<(), MarketError>
```

- **Parameters**
  - `caller` — Must be admin
  - `collection` — NFT collection contract address
  - `name` — Collection name (symbol)
  - `ipfs_uri` — IPFS metadata URI
  - `metadata_hash` — Metadata hash (32 bytes)
  - `standard` — `ERC721` or `ERC1155`
  - `royalty_recipients` — List of (address, bps) tuples

- **Authorization** — `caller` must be admin

- **Validation**
  - Collection not already registered
  - Metadata hash must be 32 bytes
  - Each royalty rate must be 0-1,000 basis points
  - Max 10 royalty recipients
  - Total royalties must not exceed 1,000 basis points

- **Errors**
  - `MarketError::CollectionAlreadyRegistered` — Collection registered
  - `MarketError::InvalidMetadataHash` — Wrong hash length
  - `MarketError::InvalidRoyaltyRate` — Rate out of range
  - `MarketError::InvalidArgument` — Too many recipients

#### `set_collection_royalties`

Update royalty config for a collection (collection admin only).

```
set_collection_royalties(
  caller: Address,
  collection: Address,
  royalty_recipients: Vec<RoyaltyRecipient>
) -> Result<(), MarketError>
```

- **Authorization** — `caller` must be collection admin

- **Errors**
  - `MarketError::Unauthorized` — Caller not collection admin
  - `MarketError::CollectionNotFound` — Collection not registered
  - `MarketError::InvalidRoyaltyRate` — Rate out of range

#### `get_collection`

Get collection information.

```
get_collection(env: Env, collection: Address) -> Result<CollectionInfo, MarketError>
```

- **Returns** — Collection metadata, royalty config, and active listing/auction counts

- **Errors**
  - `MarketError::CollectionNotFound` — Collection not registered

### Fixed-Price Listings

#### `list_nft`

Create a fixed-price listing for an NFT.

```
list_nft(
  seller: Address,
  collection: Address,
  token_id: u64,
  amount: u64,
  price: i128,
  currency: Address,
  metadata_hash: Bytes
) -> Result<u64, MarketError>
```

- **Parameters**
  - `seller` — Listing creator (must invoke)
  - `collection` — NFT collection address
  - `token_id` — NFT token ID
  - `amount` — NFT amount (≥ 1 for ERC1155)
  - `price` — Price in currency units (must be > 0)
  - `currency` — Payment currency (must be whitelisted)
  - `metadata_hash` — Metadata hash (32 bytes)

- **Authorization** — `seller` must invoke (`require_auth()`)

- **Preconditions** — Contract not paused

- **Token Transfer** — Transfers NFT from seller to marketplace (escrow)

- **Returns** — Newly created listing ID

- **Errors**
  - `MarketError::ContractPaused` — Marketplace paused
  - `MarketError::InvalidAmount` — Price ≤ 0
  - `MarketError::CurrencyNotWhitelisted` — Currency not allowed
  - `MarketError::InvalidMetadataHash` — Hash wrong length
  - `MarketError::CollectionNotFound` — Collection not registered

- **Events Emitted**
  - `NftListed` — Custom event with (listing_id, seller, collection, token_id, price, currency, timestamp)

#### `buy_nft`

Purchase a listed NFT.

```
buy_nft(env: Env, buyer: Address, listing_id: u64) -> Result<(), MarketError>
```

- **Parameters**
  - `buyer` — Purchaser (must invoke)
  - `listing_id` — ID of listing to buy

- **Authorization** — `buyer` must invoke (`require_auth()`)

- **Preconditions** — Contract not paused; listing must be active

- **Payment Flow**
  1. Calculate platform fee: `price × platform_fee_bps / 10,000`
  2. Calculate royalties from collection config
  3. Calculate seller proceeds: `price - platform_fee - royalties`
  4. Transfer platform fee to fee_recipient
  5. Transfer royalties to each recipient
  6. Transfer seller proceeds to seller
  7. Transfer NFT to buyer

- **Errors**
  - `MarketError::ListingNotFound` — Listing doesn't exist
  - `MarketError::ListingAlreadySold` — Listing not active
  - `MarketError::ContractPaused` — Marketplace paused

- **State Changes** — Listing marked `Sold`; NFT transferred

- **Events Emitted**
  - `NftSold` — Custom event with (listing_id, seller, buyer, price, timestamp)

#### `cancel_listing`

Cancel an active listing and refund NFT (seller only).

```
cancel_listing(env: Env, caller: Address, listing_id: u64) -> Result<(), MarketError>
```

- **Parameters**
  - `caller` — Must be listing seller
  - `listing_id` — ID of listing to cancel

- **Authorization** — `caller` must invoke (`require_auth()`)

- **Errors**
  - `MarketError::ListingNotFound` — Listing doesn't exist
  - `MarketError::ListingAlreadySold` — Listing not active
  - `MarketError::NotOwner` — Caller not seller

- **State Changes** — Listing marked `Cancelled`; NFT returned to seller

#### `get_listing`

Get listing details.

```
get_listing(env: Env, listing_id: u64) -> Result<Listing, MarketError>
```

- **Returns** — Listing metadata including seller, price, status, etc.

- **Errors**
  - `MarketError::ListingNotFound` — Listing doesn't exist

### English Auctions

#### `create_english_auction`

Create an English auction.

```
create_english_auction(
  seller: Address,
  collection: Address,
  token_id: u64,
  amount: u64,
  start_price: i128,
  currency: Address,
  duration_seconds: u64,
  metadata_hash: Bytes
) -> Result<u64, MarketError>
```

- **Parameters**
  - `seller` — Auction creator (must invoke)
  - `collection` — NFT collection
  - `start_price` — Opening bid (must be > 0)
  - `duration_seconds` — Auction duration (1-604,800 seconds)
  - Other parameters similar to fixed-price listing

- **Authorization** — `seller` must invoke

- **Preconditions** — Contract not paused

- **Auto-Extension** — If a bid is placed within the auto-extension window before end time, auction extends by the window duration

- **Returns** — Newly created auction ID

- **Errors** — Similar to `list_nft` plus duration validation

#### `place_bid`

Place a bid on an English auction.

```
place_bid(
  env: Env,
  bidder: Address,
  auction_id: u64,
  bid_amount: i128
) -> Result<(), MarketError>
```

- **Parameters**
  - `bidder` — Bidder address (must invoke)
  - `auction_id` — Auction to bid on
  - `bid_amount` — Bid price (must be ≥ start_price initially; then > current + increment)

- **Authorization** — `bidder` must invoke

- **Preconditions**
  - Contract not paused
  - Auction active (end_time not reached)
  - Bid meets minimum increment

- **Bid Increment** — `min_next_bid = current_bid × (1 + bid_increment_bps / 10,000)`

- **Errors**
  - `MarketError::AuctionEnded` — Auction has ended
  - `MarketError::BidTooLow` — Bid below minimum
  - `MarketError::BidderIsCurrentHighest` — Already highest bidder

- **State Changes** — Updates current bid and bidder; may extend end_time

- **Auto-Extension** — If bid placed within auto_extension_window of end_time, extend end_time by window

#### `settle_english_auction`

Settle a completed English auction (payout).

```
settle_english_auction(env: Env, auction_id: u64) -> Result<(), MarketError>
```

- **Parameters**
  - `auction_id` — Auction to settle

- **Preconditions** — Auction ended; settlement period passed

- **Payment Flow** — Similar to `buy_nft`: platform fee, royalties, seller proceeds

- **Errors**
  - `MarketError::AuctionNotYetEnded` — Auction still active

### Dutch Auctions

#### `create_dutch_auction`

Create a Dutch auction (price decay over time).

```
create_dutch_auction(
  seller: Address,
  collection: Address,
  token_id: u64,
  amount: u64,
  start_price: i128,
  floor_price: i128,
  currency: Address,
  duration_seconds: u64,
  metadata_hash: Bytes
) -> Result<u64, MarketError>
```

- **Parameters**
  - `start_price` — Initial price
  - `floor_price` — Minimum price (must be < start_price)
  - Other parameters similar to English auction

- **Price Decay** — Linear interpolation: `current_price = start_price - (start_price - floor_price) × (elapsed / duration)`

- **Errors** — Similar to English auction plus `InvalidDutchAuctionPrices`

#### `buy_dutch_auction`

Purchase from a Dutch auction at current price.

```
buy_dutch_auction(
  env: Env,
  buyer: Address,
  auction_id: u64
) -> Result<(), MarketError>
```

- **Parameters**
  - `buyer` — Purchaser (must invoke)
  - `auction_id` — Auction to purchase from

- **Authorization** — `buyer` must invoke

- **Current Price** — Calculated based on elapsed time and price decay formula

- **Preconditions** — Auction active; price > 0

- **Errors**
  - `MarketError::DutchAuctionPriceZero` — Price reached zero
  - `MarketError::AuctionEnded` — Auction time expired

### Offers

#### `make_offer`

Make an offer for an NFT (off-chain or on-chain escrow).

```
make_offer(
  offerer: Address,
  recipient: Address,
  collection: Address,
  token_id: u64,
  amount: u64,
  offer_amount: i128,
  currency: Address,
  offer_nft_collection: Address,
  offer_nft_token_id: u64,
  expires_in_seconds: u64,
  metadata_hash: Bytes
) -> Result<u64, MarketError>
```

- **Parameters**
  - `offerer` — Creator of offer (must invoke)
  - `recipient` — Intended recipient
  - `collection` / `token_id` — NFT being offered for
  - `offer_amount` — Offer price
  - `currency` — Payment currency
  - `offer_nft_collection` / `offer_nft_token_id` — NFT offered (if counter-offer)
  - `expires_in_seconds` — Offer duration

- **Authorization** — `offerer` must invoke

- **Returns** — Newly created offer ID

#### `accept_offer`

Accept an offer (recipient only).

```
accept_offer(env: Env, caller: Address, offer_id: u64) -> Result<(), MarketError>
```

- **Authorization** — `caller` must be recipient

- **Preconditions**
  - Offer not expired
  - Offer not already settled

- **Errors**
  - `MarketError::OfferExpired` — Offer expired
  - `MarketError::OfferAlreadySettled` — Already accepted/cancelled
  - `MarketError::NotOfferRecipient` — Caller not recipient

#### `cancel_offer`

Cancel an offer (offerer only).

```
cancel_offer(env: Env, caller: Address, offer_id: u64) -> Result<(), MarketError>
```

- **Authorization** — `caller` must be offerer or recipient (depending on implementation)

### Bulk Operations

#### `bulk_list_nfts`

List multiple NFTs in a single transaction.

```
bulk_list_nfts(
  env: Env,
  seller: Address,
  items: Vec<BulkListingItem>
) -> Result<BulkResult, MarketError>
```

- **Parameters** — Vector of (collection, token_id, amount, price, currency, metadata_hash)

- **Returns** — `BulkResult { succeeded, failed, ids: Vec<u64> }`

- **Max Items** — 50 per call

#### `bulk_create_auctions`

Create multiple auctions in a single transaction.

```
bulk_create_auctions(
  env: Env,
  seller: Address,
  items: Vec<BulkAuctionItem>
) -> Result<BulkResult, MarketError>
```

- **Parameters** — Vector of auction specs

- **Max Items** — 50 per call

---

## oracle-contract

Stub oracle contract for integration with external price feeds and verification services.

### Initialization

#### `initialize`

Initialize the oracle contract (required before other calls; admin only).

```
initialize(env: Env, admin: Address)
```

- **Parameters**
  - `admin` — Admin address with governance privileges

- **Events Emitted**
  - `ModuleInitialized` — `("logging", "initialized")` with module name, version, caller, timestamp

**Note** — This is a minimal stub contract. Production deployments should extend it with price feed aggregation, signature verification, and data validation logic. See the Trellis architecture documentation for oracle design patterns.

---

## rebalancer-contract

(Placeholder for rebalancer-contract documentation — not yet implemented in this codebase)

---

# Events

Contracts emit events for:

AidCreated

AidClaimed

AidSettled

AidRefunded

CommissionPaid

TreasuryDeposit

TreasuryWithdrawal

ContractPaused

ContractResumed

ContractUpgraded

ModuleInitialized

ActionExecuted

PermissionChanged

Off-chain indexers should match the stable two-part topic tuples and decode the typed payloads documented in `shared/README.md`.

---

# Documentation

This repository contains comprehensive documentation for contract design, gas optimization, security practices, and testing infrastructure.

## Core Documentation

- [**UPGRADEABILITY.md**](UPGRADEABILITY.md) — System-wide upgrade registry and safe contract upgrade patterns with migration hooks and pre/post-upgrade validation.
- [**GAS_OPTIMIZATION.md**](GAS_OPTIMIZATION.md) — Gas optimization principles, applied optimizations, benchmark infrastructure, and a checklist for new features.
- [**SECURITY_BATCH.md**](SECURITY_BATCH.md) — Security analysis of batch operations including reentrancy protection, input validation, and failure semantics.

## Module Documentation

- [**shared/README.md**](shared/README.md) — Shared contract library with error codes, reserved ranges, and event schemas used across all contracts.
- [**testing/README.md**](testing/README.md) — Comprehensive testing framework with mocks, helpers, simulation tools, fuzzing harnesses, and examples.
- [**security/README.md**](security/README.md) — Security audit tooling, CI gating, and allowlist management for static analysis and vulnerability scanning.

---

# Future Roadmap

- Cross-chain settlement
- Stellar Asset support
- Multi-token donations
- DAO governance
- Zero Knowledge verification
- On-chain reputation
- Human identity proofs
- Streaming donations
- Batch settlements

---

# Contributing

We welcome contributions from Rust and Soroban developers.

Read [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide: local setup, build and test instructions, branch and commit conventions, and what reviewers look for.

Please note that this project is released with a [Contributor Code of Conduct](CODE_OF_CONDUCT.md). By participating you agree to abide by its terms.

Workflow

1. Fork repository

2. Create feature branch

3. Write tests

4. Submit Pull Request

Every contract contribution must include:

- Unit tests

- Documentation

- Security considerations

- Gas optimization review

---

# Brand

The Trellis mark, wordmark and palette live in [`brand/`](brand/) — SVG, PNG and ICO
variants for light and dark backgrounds, plus clear-space and minimum-size rules.

| Vine      | Amber     | Ink       | Paper     |
| --------- | --------- | --------- | --------- |
| `#1C6B55` | `#E39A3C` | `#14201C` | `#F7F5F0` |

Vine is the structure, amber is the accent — one amber element per composition.

---

# License

Licensed under the MIT License.

See LICENSE for details.

---

# Built With

- Stellar
- Soroban
- Rust
- Open Source Community

Building transparent humanitarian infrastructure for everyone.
