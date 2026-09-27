# Trellis Operational Runbook

Incident triage and emergency rollback for the Trellis Soroban contracts.
This runbook is the single entry point on-call engineers follow when a
Trellis deployment misbehaves. It lists the incident categories, the triage
commands, the decision points, and the rollback / mitigation paths.

> **Never** paste secret keys, RPC tokens, or production credentials into an
> incident channel, PR, or shell history. Source configuration from the
> environment as described in [`CONFIGURATION.md`](./CONFIGURATION.md) and read
> secret redaction guidance in [`../SECURITY.md`](../SECURITY.md).

---

## 1. First five minutes

1. **Acknowledge** the alert and record the start time in the incident channel.
2. **Classify** using the table in §2.
3. **Freeze** non-incident deploys: pause merges to `main` and stop any
   in-flight `scripts/deploy.sh` runs.
4. **Capture state** before changing anything:

   ```bash
   # Repo health: formatting, lint, tests, build (same gates as CI).
   ./scripts/verify.sh

   # Contract/config diagnostics (RPC reachability, network id, flags).
   ./scripts/diagnostics.sh

   # Confirm the deployed network + contract ids recorded at deploy time.
   cat DEPLOYMENTS.md
   ```

5. **Open a timeline** and paste the outputs above (redact endpoints that embed
   credentials).

---

## 2. Incident categories

| Category | Typical signal | Primary risk | First action |
|---|---|---|---|
| Pause / liveness | Entry points revert with `ContractPaused` (5) | Funds frozen, no side effects | §3.1 |
| Authorization | `Unauthorized` (1) spikes, `role` errors | Unauthorized writes | §3.2 |
| Accounting / amounts | `InvalidAmount` (3), `Overflow` (4) | Mis-settlement, drift | §3.3 |
| Schema / migration | `UnsupportedSchemaVersion` (20), `SchemaMigrationFailed` (21) | Unreadable records | §3.4 |
| Upgrade | `UpgradeAlreadyPending` (906), `MigrationHookFailed` (905) | Stuck release | §3.5 |
| Escrow / payments | `PaymentEscrow*` (700–710) | Stuck or double release | §3.6 |
| Quota / abuse | `QuotaExceeded` (22) by many actors | Legit users blocked | §3.7 |

Error codes are defined in [`shared/src/errors.rs`](../shared/src/errors.rs).

---

## 3. Triage procedures

### 3.1 Pause / liveness

**Decision:** is the contract intentionally paused, or paused by a bad release?

```bash
# Read-only: is the pause flag set? (getter name from the target contract)
soroban contract invoke \
  --id "$CONTRACT_ID" --network "$NETWORK" --source-account "$READ_ONLY_ACCOUNT" \
  -- is_paused
```

- **If paused unintentionally after a release** → treat as a bad release, go to
  §4 (Emergency rollback).
- **If pause was intentional** → confirm the owner (`get_admin`) and the
  expected unpause date, then communicate status.

### 3.2 Authorization

```bash
# Who is the admin?
soroban contract invoke --id "$CONTRACT_ID" --network "$NETWORK" \
  --source-account "$READ_ONLY_ACCOUNT" -- get_admin

# Does an account hold a role? (role enum per shared/src/auth.rs)
soroban contract invoke --id "$CONTRACT_ID" --network "$NETWORK" \
  --source-account "$READ_ONLY_ACCOUNT" -- has_role --user "$ADDRESS" --role Admin
```

**Decision:** if the admin is wrong or a role grant is missing, re-grant via the
admin path (`grant_role`), which requires the current admin signature. If the
admin key is suspected compromised, follow [`../SECURITY.md`](../SECURITY.md).

### 3.3 Accounting / amounts

```bash
# Reproduce the arithmetic path locally against the exact release.
git checkout "$RELEASE_TAG"
cargo test -p shared math:: -- --nocapture
cargo test -p payments-contract -- --nocapture
```

Check `shared/src/math.rs` helpers (`checked_add`, `checked_mul`, `apply_bps`)
before assuming a contract bug — most `Overflow` reports are pre-condition
violations. If a settlement is wrong, stop withdrawals (§3.6) and reconcile.

### 3.4 Schema / migration

```bash
cargo test -p shared compat:: -- --nocapture
```

- `UnsupportedSchemaVersion` on read → the reader is older than the written
  record. Redeploy the current build; do **not** hand-edit ledger entries.
- `SchemaMigrationFailed` on write → validate the record shape and re-run the
  migration path described in [`COMPATIBILITY.md`](./COMPATIBILITY.md).

### 3.5 Upgrade

```bash
# What is registered / pending?
cargo test -p upgradeability -- --nocapture
./scripts/register_upgradeable.sh --help
```

**Decision:** if an upgrade is pending but stuck, complete or cancel it using
the registry flow in [`../UPGRADEABILITY.md`](../UPGRADEABILITY.md). Never
re-register a contract under a different name to work around a stuck upgrade.

### 3.6 Escrow / payments (stop the bleeding)

If value is at risk, prefer **pause + no new releases** over ad-hoc transfers:

```bash
# Pause state-changing paths (admin only).
soroban contract invoke --id "$CONTRACT_ID" --network "$NETWORK" \
  --source-account "$ADMIN_IDENTITY" -- set_paused --paused true
```

Then reconcile each escrow read-only before resuming:

```bash
soroban contract invoke --id "$CONTRACT_ID" --network "$NETWORK" \
  --source-account "$READ_ONLY_ACCOUNT" -- get_escrow --id "$ESCROW_ID"
```

### 3.7 Quota / abuse

```bash
# Inspect config + usage for a resource (see docs/QUOTA.md).
soroban contract invoke --id "$CONTRACT_ID" --network "$NETWORK" \
  --source-account "$READ_ONLY_ACCOUNT" -- get_quota_status \
  --actor "$ADDRESS" --resource aid_crt
```

**Decision:** reset a falsely-blocked actor (`reset_quota`) or raise limits with
`set_quota_config`. Treat a broad `QuotaExceeded` spike as an abuse signal and
escalate to security.

---

## 4. Persistent storage TTL renewal

Persistent records must use the helpers exported by `shared::storage`:

- `persistent_set` writes the value and bumps it to `PERSISTENT_BUMP_AMOUNT`.
- `persistent_get` reads the value and refreshes the TTL when the key exists.
- `persistent_read` is reserved for an intermediate read that is guaranteed to
  be written in the same transaction; it must not be used for public query
  paths that need to keep records alive.
- `persistent_has` is an existence check only. Follow it with `persistent_get`
  when the record is needed.

Do not call `env.storage().persistent().set`, `.get`, or `.extend_ttl` directly
from a production contract. A new persistent key must be covered by a test
that advances the ledger close to `PERSISTENT_TTL_THRESHOLD`, reads or writes
the key through the helper, and verifies that the entry remains available.
The current seven-day bump and six-day refresh threshold are defined in
`shared/src/storage.rs`; change those constants deliberately and review the
gas impact before deployment.

## 5. Emergency rollback

Use when a release causes incorrect state changes or blocks critical
operations. Rollback is an **upgrade to the last known-good WASM**, plus the
documented migration/compat handling.

1. **Pause first** so no new side effects land during the rollback (§3.6).
2. **Identify the last known-good artifact** from `DEPLOYMENTS.md` and the
   release tag. *This is a decision point:* if state written by the bad release
   is not backward-compatible, stop and get the contract owner's sign-off before
   proceeding.
3. **Register and execute the rollback upgrade** following
   [`../UPGRADEABILITY.md`](../UPGRADEABILITY.md) and
   [`../DEPLOYMENTS.md`](../DEPLOYMENTS.md):

   ```bash
   # 1) re-register the known-good WASM hash
   ./scripts/register_upgradeable.sh --network "$NETWORK" --wasm "$GOOD_WASM"

   # 2) execute the upgrade (owner/admin signs)
   ./scripts/upgrade.sh --network "$NETWORK" --wasm-hash "$GOOD_WASM_HASH"

   # 3) record the new deployment for the next incident
   ./scripts/record-deployments.sh --network "$NETWORK" --note "rollback <incident>"
   ```

4. **Validate** with a read-only smoke test before unpausing:

   ```bash
   ./scripts/verify.sh
   ./scripts/diagnostics.sh
   ```

5. **Unpause** only after the smoke test passes, and post the outcome in §5.

> If an upgrade cannot be executed (e.g. no owner signature available), the
> mitigation is to **stay paused** and communicate. Do not move funds manually.

---

## 5. Communication template

```
INCIDENT <id> — <category> — <SEV>
Start (UTC):      <timestamp>
Impact:           <who/what>
Detected by:      <alert | report>
Current status:   <investigating | mitigated | resolved>
Last update:      <timestamp>
Next update:      <timestamp, <= 30 min>
Mitigation:       <pause | rollback | config change>
Owner:            @<on-call>
```

Update at least every 30 minutes until resolved, then publish a short
post-incident note: root cause, detection gap, and follow-up issues.

---

## 6. Reference

- [`DIAGNOSTICS.md`](./DIAGNOSTICS.md) — diagnostic tooling details.
- [`QUOTA.md`](./QUOTA.md) — quota limits and override procedure.
- [`COMPATIBILITY.md`](./COMPATIBILITY.md) — schema versions and migration.
- [`CONFIGURATION.md`](./CONFIGURATION.md) — env/secret requirements.
- [`../UPGRADEABILITY.md`](../UPGRADEABILITY.md) — upgrade registry and rollback.
- [`../DEPLOYMENTS.md`](../DEPLOYMENTS.md) — deployed contract ids per network.
- [`../SECURITY.md`](../SECURITY.md) — disclosure and key-compromise steps.
