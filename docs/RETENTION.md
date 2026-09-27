# Data Retention Policy (Issue #45)

Operational data — audit trails, telemetry, exports, and support evidence —
must be kept long enough for audits and support, but must not accumulate
forever. The retention module makes that behaviour **explicit, reviewable, and
testable** instead of ad hoc.

The implementation lives in
[`shared/src/retention.rs`](../shared/src/retention.rs) and is re-exported from
the `shared` crate root.

## Data classification

Every tracked record belongs to a `DataClass`, and each class maps to a record
kind symbol so callers can classify without hard-coding numbers:

| `DataClass` | `classify` symbol | Default window | Archive first | Delete frozen |
|---|---|---|---|---|
| `Audit` | `audit` | 365 days | yes | no |
| `Telemetry` | `telemetry` | 30 days | no | no |
| `Export` | `export` | 180 days | yes | no |
| `SupportEvidence` | `support` | 90 days | no | no |
| `FinancialSettlement` | `settle` | 2 555 days (~7 years) | yes | **yes** |
| `Dispute` | `dispute` | 365 days | yes | **yes** |

Windows are expressed in ledgers at the repository's ~5 s/ledger cadence
(`LEDGERS_PER_DAY = 17_280`), and `days_to_ledgers` converts whole days.
Unknown record kinds return `Error::NotFound` rather than being classified
into the wrong bucket.

A class with no stored policy falls back to its compile-time default — absence
of configuration never means "delete".

## Protection rules

Records are protected from cleanup in two independent ways:

1. **Active hold** — `place_hold(env, id, reason, until_ledger)` with a reason
   of `Dispute`, `Audit`, `Settlement`, or `Legal`. A hold with
   `until_ledger == HOLD_INDEFINITE` (`0`) never lapses. A hold always wins
   over expiry, and holds may not be placed in the past
   (`Error::ConfigInvalid`).
2. **Frozen class** — `FinancialSettlement` and `Dispute` are frozen by
   default, because deleting them could destroy evidence tied to a live
   obligation. A frozen class is never auto-deleted regardless of age.

`release_hold` removes a hold and returns `Error::NotFound` when there is
nothing to release, so idempotent callers can ignore that case explicitly.

## Report-first cleanup

Cleanup is always two-phase. It never deletes anything the maintainer has not
seen first.

```rust
// 1. Plan (non-destructive): what would change, and what is protected.
let plan = plan_cleanup(&env, &records);
//    plan.eligible          -> expired, unprotected
//    plan.protected         -> expired, covered by an active hold
//    plan.frozen            -> frozen class, never auto-deleted
//    plan.retained          -> not yet past its window
//    plan.bytes_reclaimable / bytes_protected

// 2. Review, then execute (requires an explicit confirmation).
let report = apply_cleanup(&env, &plan, true)?;
//    report.deleted            -> records removed
//    report.skipped_protected  -> plan.protected + plan.frozen + late protections
//    report.bytes_reclaimed
```

Behaviour guarantees:

- A dry-run plan (`plan.dry_run == true`) is a **no-op** report — it can never
  delete.
- A real run (`dry_run == false`) requires `confirm == true`, otherwise it
  fails with `Error::InvalidArgument`.
- Protection is re-checked **per record at execution time**, so a hold placed
  after the plan was produced still wins. Such records appear in
  `CleanupReport::skipped_protected` instead of failing the whole run.
- A hand-forged plan cannot delete a frozen class: the frozen check happens
  again during execution.
- `plan_cleanup_at(env, records, evaluated_ledger)` pins the evaluation ledger
  so plans are reproducible in tests and in review.

## Storage layout

| Key | Storage | Value |
|---|---|---|
| `RetentionKey::Policy(DataClass)` | instance | `RetentionPolicy` |
| `RetentionKey::Hold(Symbol)` | persistent | `RetentionHold` |
| `RetentionKey::Record(Symbol)` | persistent | `RetentionRecord` |

Records are registered with `put_record`; policies are set with
`set_retention_policy` (called from an admin-gated entry point — this module
validates and persists only). TTLs use the shared storage helpers, so tracked
records follow the repository's standard persistent bump policy.

## Deployment notes

- No migration is required. Keys live in a new `RetentionKey` namespace and do
  not collide with existing contract state.
- Defaults apply immediately: until a maintainer stores an override, each
  class uses its default window from the table above.
- Because absent configuration means "retain", rolling this out cannot delete
  data on its own — deletion only happens through a confirmed
  `apply_cleanup`.
- Contracts adopting retention should expose `plan_cleanup` (read-only) and
  `apply_cleanup` (admin-gated) as entry points so operators can review before
  anything is removed.

## Validation

```bash
cargo test -p shared retention
```

The suite covers: classification, default/override policies, dry-run
non-destructiveness, the confirmation requirement, protected vs. eligible
records, holds placed after planning, lapsed and released holds, frozen
classes surviving a forged plan, and invalid inputs.
