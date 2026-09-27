//! Data retention policy for operational data (Issue #45).
//!
//! Operational data — audit trails, telemetry, exports, and support evidence —
//! must be kept long enough for audits and support, but must not accumulate
//! forever. This module makes retention **explicit, reviewable, and testable**.
//!
//! ## Design
//!
//! - Every record is classified into a [`DataClass`]. Each class has a
//!   [`RetentionPolicy`] (retention window in ledgers, archive-first flag,
//!   hold honoring, deletion freeze).
//! - Absent policy is **fail-safe**: [`get_effective_policy`] falls back to the
//!   compile-time default for the class, so data is never deleted just because
//!   a maintainer has not configured the class yet.
//! - Records linked to an active dispute, audit, settlement, or legal hold are
//!   protected via [`place_hold`]. A hold always wins over expiry.
//! - Deletion is **report-first**: [`plan_cleanup`] returns a non-destructive
//!   [`CleanupPlan`]; [`apply_cleanup`] refuses to act on a dry-run plan and
//!   requires an explicit `confirm`, and it re-checks holds at apply time so a
//!   hold placed after planning still wins.
//! - A cleanup that skips a protected record reports it in
//!   [`CleanupReport::skipped_protected`] instead of failing the whole run.
//!
//! See `docs/RETENTION.md` for the classification table, defaults, and the
//! maintainer runbook.

use soroban_sdk::{contracttype, symbol_short, Env, Symbol, Vec};

use crate::errors::Error;
use crate::storage::{
    instance_get, instance_set, persistent_get, persistent_has, persistent_remove, persistent_set,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Ledgers per day at the ~5 s/ledger cadence the repository targets.
pub const LEDGERS_PER_DAY: u32 = 17_280;

/// Hard ceiling on any configured retention window (10 years).
pub const MAX_RETAIN_LEDGERS: u32 = LEDGERS_PER_DAY * 3_650;

/// Sentinel `until_ledger` value meaning "hold indefinitely".
pub const HOLD_INDEFINITE: u32 = 0;

/// Convert whole days into ledgers (saturating at [`MAX_RETAIN_LEDGERS`]).
pub const fn days_to_ledgers(days: u32) -> u32 {
    let ledgers = days.saturating_mul(LEDGERS_PER_DAY);
    if ledgers > MAX_RETAIN_LEDGERS {
        MAX_RETAIN_LEDGERS
    } else {
        ledgers
    }
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

/// Operational data classes governed by retention rules.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataClass {
    /// Audit trail of privileged / administrative actions.
    Audit,
    /// Runtime telemetry: metrics, traces, health samples.
    Telemetry,
    /// User- or maintainer-requested data exports.
    Export,
    /// Evidence attached to support tickets.
    SupportEvidence,
    /// Records tied to financial settlement (escrows, fees, payouts).
    FinancialSettlement,
    /// Records attached to an open or historical dispute.
    Dispute,
}

/// Why a record is protected from deletion.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HoldReason {
    /// Open dispute; deletion would destroy evidence.
    Dispute,
    /// Active or scheduled compliance / security audit.
    Audit,
    /// Unsettled financial obligation tied to the record.
    Settlement,
    /// Legal or regulatory preservation order.
    Legal,
}

/// Map a record-kind symbol onto a [`DataClass`].
///
/// Unknown kinds map to [`Error::NotFound`] so a caller never silently
/// classifies data into the wrong retention bucket.
pub fn classify(kind: &Symbol) -> Result<DataClass, Error> {
    if kind == &symbol_short!("audit") {
        Ok(DataClass::Audit)
    } else if kind == &symbol_short!("telemetry") {
        Ok(DataClass::Telemetry)
    } else if kind == &symbol_short!("export") {
        Ok(DataClass::Export)
    } else if kind == &symbol_short!("support") {
        Ok(DataClass::SupportEvidence)
    } else if kind == &symbol_short!("settle") {
        Ok(DataClass::FinancialSettlement)
    } else if kind == &symbol_short!("dispute") {
        Ok(DataClass::Dispute)
    } else {
        Err(Error::NotFound)
    }
}

/// Short stable symbol for a class (reports, logs, dashboards).
pub fn class_label(class: DataClass) -> Symbol {
    match class {
        DataClass::Audit => symbol_short!("audit"),
        DataClass::Telemetry => symbol_short!("telemetry"),
        DataClass::Export => symbol_short!("export"),
        DataClass::SupportEvidence => symbol_short!("support"),
        DataClass::FinancialSettlement => symbol_short!("settle"),
        DataClass::Dispute => symbol_short!("dispute"),
    }
}

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// Retention rules for one [`DataClass`].
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionPolicy {
    /// Ledgers a record of this class is kept from its creation ledger.
    pub retain_ledgers: u32,
    /// Move the record to the archive lane before deleting (export first).
    pub archive_before_delete: bool,
    /// Active holds always win over expiry (must stay `true` in practice).
    pub honor_holds: bool,
    /// Class is frozen: never auto-delete (settlement and dispute evidence).
    pub deletion_frozen: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            retain_ledgers: days_to_ledgers(30),
            archive_before_delete: true,
            honor_holds: true,
            deletion_frozen: false,
        }
    }
}

impl RetentionPolicy {
    /// Validate ranges before persisting.
    pub fn validate(&self) -> Result<(), Error> {
        if self.retain_ledgers == 0 || self.retain_ledgers > MAX_RETAIN_LEDGERS {
            return Err(Error::ConfigInvalid);
        }
        Ok(())
    }
}

/// Compile-time default policy for a class — the ground truth when no
/// maintainer override is stored.
pub fn default_policy(class: DataClass) -> RetentionPolicy {
    match class {
        DataClass::Audit => RetentionPolicy {
            retain_ledgers: days_to_ledgers(365),
            archive_before_delete: true,
            honor_holds: true,
            deletion_frozen: false,
        },
        DataClass::Telemetry => RetentionPolicy {
            retain_ledgers: days_to_ledgers(30),
            archive_before_delete: false,
            honor_holds: true,
            deletion_frozen: false,
        },
        DataClass::Export => RetentionPolicy {
            retain_ledgers: days_to_ledgers(180),
            archive_before_delete: true,
            honor_holds: true,
            deletion_frozen: false,
        },
        DataClass::SupportEvidence => RetentionPolicy {
            retain_ledgers: days_to_ledgers(90),
            archive_before_delete: false,
            honor_holds: true,
            deletion_frozen: false,
        },
        DataClass::FinancialSettlement => RetentionPolicy {
            retain_ledgers: days_to_ledgers(2_555),
            archive_before_delete: true,
            honor_holds: true,
            deletion_frozen: true,
        },
        DataClass::Dispute => RetentionPolicy {
            retain_ledgers: days_to_ledgers(365),
            archive_before_delete: true,
            honor_holds: true,
            deletion_frozen: true,
        },
    }
}

/// A tracked record eligible for retention evaluation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionRecord {
    pub id: Symbol,
    pub class: DataClass,
    /// Ledger the record was created at.
    pub created_ledger: u32,
    /// Approximate on-chain footprint, used for reclaim reporting.
    pub bytes: u32,
}

/// A protection order on a single record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionHold {
    pub id: Symbol,
    pub reason: HoldReason,
    /// Ledger the hold lapses at; [`HOLD_INDEFINITE`] means never.
    pub until_ledger: u32,
    pub placed_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum RetentionKey {
    Policy(DataClass),
    Hold(Symbol),
    Record(Symbol),
}

fn policy_key(class: DataClass) -> RetentionKey {
    RetentionKey::Policy(class)
}

fn hold_key(id: &Symbol) -> RetentionKey {
    RetentionKey::Hold(id.clone())
}

fn record_key(id: &Symbol) -> RetentionKey {
    RetentionKey::Record(id.clone())
}

// ---------------------------------------------------------------------------
// Policy / hold / record administration
// ---------------------------------------------------------------------------

/// Store an override policy for a class. Admin authorization is enforced by
/// the calling contract entry point; this helper only validates and persists.
pub fn set_retention_policy(
    env: &Env,
    class: DataClass,
    policy: &RetentionPolicy,
) -> Result<(), Error> {
    policy.validate()?;
    instance_set(env, &policy_key(class), policy);
    Ok(())
}

/// Stored override policy for a class, if a maintainer set one.
pub fn get_retention_policy(env: &Env, class: DataClass) -> Option<RetentionPolicy> {
    instance_get(env, &policy_key(class))
}

/// Policy actually used for a class: stored override, else class default.
///
/// Never returns `None`: absence of configuration must not mean "delete".
pub fn get_effective_policy(env: &Env, class: DataClass) -> RetentionPolicy {
    get_retention_policy(env, class).unwrap_or_else(|| default_policy(class))
}

/// Register (or refresh) a record so cleanup can later act on it.
pub fn put_record(env: &Env, record: &RetentionRecord) -> Result<(), Error> {
    if record.created_ledger == 0 {
        return Err(Error::InvalidArgument);
    }
    persistent_set(env, &record_key(&record.id), record);
    Ok(())
}

/// Tracked record by id.
pub fn get_record(env: &Env, id: &Symbol) -> Option<RetentionRecord> {
    persistent_get(env, &record_key(id))
}

/// Place (or replace) a hold on a record.
///
/// `until_ledger` must be [`HOLD_INDEFINITE`] or in the future; a hold that has
/// already lapsed is rejected so operators cannot "protect" expired evidence
/// by mistake.
pub fn place_hold(
    env: &Env,
    id: &Symbol,
    reason: HoldReason,
    until_ledger: u32,
) -> Result<RetentionHold, Error> {
    let now = env.ledger().sequence();
    if until_ledger != HOLD_INDEFINITE && until_ledger <= now {
        return Err(Error::ConfigInvalid);
    }
    let hold = RetentionHold {
        id: id.clone(),
        reason,
        until_ledger,
        placed_ledger: now,
    };
    persistent_set(env, &hold_key(id), &hold);
    Ok(hold)
}

/// Release a hold. Idempotent callers should tolerate [`Error::NotFound`].
pub fn release_hold(env: &Env, id: &Symbol) -> Result<(), Error> {
    if !persistent_has(env, &hold_key(id)) {
        return Err(Error::NotFound);
    }
    persistent_remove(env, &hold_key(id));
    Ok(())
}

/// Stored hold for a record, active or lapsed.
pub fn get_hold(env: &Env, id: &Symbol) -> Option<RetentionHold> {
    persistent_get(env, &hold_key(id))
}

/// The active hold protecting a record at `now_ledger`, if any.
pub fn active_hold(env: &Env, id: &Symbol, now_ledger: u32) -> Option<RetentionHold> {
    match get_hold(env, id) {
        Some(hold) if hold.until_ledger == HOLD_INDEFINITE || hold.until_ledger > now_ledger => {
            Some(hold)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Cleanup planning / execution
// ---------------------------------------------------------------------------

/// One record carried inside a [`CleanupPlan`] bucket.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupEntry {
    pub id: Symbol,
    pub class: DataClass,
    pub bytes: u32,
}

/// Non-destructive description of what a cleanup run would do.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupPlan {
    /// Always `true` for a plan produced by [`plan_cleanup`].
    pub dry_run: bool,
    pub evaluated_ledger: u32,
    /// Expired, unprotected: would be deleted.
    pub eligible: Vec<CleanupEntry>,
    /// Expired but covered by an active hold.
    pub protected: Vec<CleanupEntry>,
    /// Belongs to a `deletion_frozen` class: never auto-deleted.
    pub frozen: Vec<CleanupEntry>,
    /// Not yet past its retention window.
    pub retained: Vec<CleanupEntry>,
    /// Bytes freed if `eligible` were deleted.
    pub bytes_reclaimable: u32,
    /// Bytes still pinned by holds or freezes.
    pub bytes_protected: u32,
}

/// Outcome of an [`apply_cleanup`] run.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupReport {
    pub evaluated_ledger: u32,
    /// Records actually removed from storage.
    pub deleted: Vec<Symbol>,
    /// Records left in place by protection: the plan's `protected` and
    /// `frozen` buckets, plus any eligible record that became protected
    /// between planning and execution.
    pub skipped_protected: Vec<Symbol>,
    pub bytes_reclaimed: u32,
    /// `false` for dry runs (nothing was destroyed).
    pub destructive: bool,
}

/// Build a review-first plan at an explicit ledger (deterministic in tests).
pub fn plan_cleanup_at(
    env: &Env,
    records: &Vec<RetentionRecord>,
    evaluated_ledger: u32,
) -> CleanupPlan {
    let mut eligible = Vec::new(env);
    let mut protected = Vec::new(env);
    let mut frozen = Vec::new(env);
    let mut retained = Vec::new(env);
    let mut bytes_reclaimable: u32 = 0;
    let mut bytes_protected: u32 = 0;

    let count = records.len();
    for i in 0..count {
        let Some(record) = records.get(i) else {
            continue;
        };
        let policy = get_effective_policy(env, record.class);
        let entry = CleanupEntry {
            id: record.id.clone(),
            class: record.class,
            bytes: record.bytes,
        };
        if policy.deletion_frozen {
            bytes_protected = bytes_protected.saturating_add(record.bytes);
            frozen.push_back(entry);
            continue;
        }
        if !record_expired(&record, &policy, evaluated_ledger) {
            retained.push_back(entry);
            continue;
        }
        if policy.honor_holds && active_hold(env, &record.id, evaluated_ledger).is_some() {
            bytes_protected = bytes_protected.saturating_add(record.bytes);
            protected.push_back(entry);
            continue;
        }
        bytes_reclaimable = bytes_reclaimable.saturating_add(record.bytes);
        eligible.push_back(entry);
    }

    CleanupPlan {
        dry_run: true,
        evaluated_ledger,
        eligible,
        protected,
        frozen,
        retained,
        bytes_reclaimable,
        bytes_protected,
    }
}

/// Build a plan against the current ledger.
pub fn plan_cleanup(env: &Env, records: &Vec<RetentionRecord>) -> CleanupPlan {
    plan_cleanup_at(env, records, env.ledger().sequence())
}

/// Execute a plan.
///
/// - A dry-run plan is a no-op report — never destructive.
/// - A real run requires `confirm == true`; this is the second half of the
///   "report before destructive action" contract.
/// - Protection is re-checked per record, so a hold placed *after* planning
///   still wins. Skipped records are reported, not silently dropped.
pub fn apply_cleanup(env: &Env, plan: &CleanupPlan, confirm: bool) -> Result<CleanupReport, Error> {
    if plan.dry_run {
        return Ok(CleanupReport {
            evaluated_ledger: plan.evaluated_ledger,
            deleted: Vec::new(env),
            skipped_protected: Vec::new(env),
            bytes_reclaimed: 0,
            destructive: false,
        });
    }
    if !confirm {
        return Err(Error::InvalidArgument);
    }

    let now = env.ledger().sequence();
    let mut deleted = Vec::new(env);
    let mut skipped_protected = Vec::new(env);
    let mut bytes_reclaimed: u32 = 0;

    // Everything the plan already flagged as protected stays put, and is
    // reported so the caller sees the full picture in one place.
    for i in 0..plan.protected.len() {
        if let Some(entry) = plan.protected.get(i) {
            skipped_protected.push_back(entry.id.clone());
        }
    }
    for i in 0..plan.frozen.len() {
        if let Some(entry) = plan.frozen.get(i) {
            skipped_protected.push_back(entry.id.clone());
        }
    }

    let count = plan.eligible.len();
    for i in 0..count {
        let Some(entry) = plan.eligible.get(i) else {
            continue;
        };
        if is_frozen(env, entry.class) || active_hold(env, &entry.id, now).is_some() {
            skipped_protected.push_back(entry.id.clone());
            continue;
        }
        if persistent_has(env, &record_key(&entry.id)) {
            persistent_remove(env, &record_key(&entry.id));
        }
        bytes_reclaimed = bytes_reclaimed.saturating_add(entry.bytes);
        deleted.push_back(entry.id.clone());
    }

    Ok(CleanupReport {
        evaluated_ledger: plan.evaluated_ledger,
        deleted,
        skipped_protected,
        bytes_reclaimed,
        destructive: true,
    })
}

/// Whether a class is currently frozen against auto-deletion.
pub fn is_frozen(env: &Env, class: DataClass) -> bool {
    get_effective_policy(env, class).deletion_frozen
}

fn record_expired(record: &RetentionRecord, policy: &RetentionPolicy, now: u32) -> bool {
    now.saturating_sub(record.created_ledger) >= policy.retain_ledgers
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Ledger};

    /// Minimal contract so unit tests can reach storage-backed helpers:
    /// Soroban only permits storage access inside a contract context.
    #[contract]
    pub struct RetentionHarness;

    #[contractimpl]
    impl RetentionHarness {
        pub fn noop(_env: Env) {}
    }

    fn with_storage<T>(env: &Env, f: impl FnOnce() -> T) -> T {
        let id = env.register_contract(None, RetentionHarness);
        env.as_contract(&id, f)
    }

    fn record(env: &Env, id: &str, class: DataClass, created: u32, bytes: u32) -> RetentionRecord {
        let rec = RetentionRecord {
            id: Symbol::new(env, id),
            class,
            created_ledger: created,
            bytes,
        };
        put_record(env, &rec).unwrap();
        rec
    }

    #[test]
    fn classify_maps_known_kinds_and_rejects_unknown() {
        assert_eq!(classify(&symbol_short!("audit")), Ok(DataClass::Audit));
        assert_eq!(
            classify(&symbol_short!("telemetry")),
            Ok(DataClass::Telemetry)
        );
        assert_eq!(classify(&symbol_short!("export")), Ok(DataClass::Export));
        assert_eq!(
            classify(&symbol_short!("support")),
            Ok(DataClass::SupportEvidence)
        );
        assert_eq!(
            classify(&symbol_short!("settle")),
            Ok(DataClass::FinancialSettlement)
        );
        assert_eq!(classify(&symbol_short!("dispute")), Ok(DataClass::Dispute));
        assert_eq!(classify(&symbol_short!("nope")), Err(Error::NotFound));
    }

    #[test]
    fn defaults_cover_every_class_and_freeze_settlement_evidence() {
        let env = Env::default();
        with_storage(&env, || {
            // Settlement and dispute evidence are frozen by default.
            assert!(is_frozen(&env, DataClass::FinancialSettlement));
            assert!(is_frozen(&env, DataClass::Dispute));
            assert!(!is_frozen(&env, DataClass::Telemetry));
            // Telemetry has the shortest default window; audit the longest.
            assert_eq!(
                get_effective_policy(&env, DataClass::Telemetry).retain_ledgers,
                days_to_ledgers(30)
            );
            assert!(
                get_effective_policy(&env, DataClass::Audit).retain_ledgers
                    > get_effective_policy(&env, DataClass::Telemetry).retain_ledgers
            );
            assert_eq!(
                class_label(DataClass::SupportEvidence),
                symbol_short!("support")
            );
        });
    }

    #[test]
    fn stored_override_wins_over_default() {
        let env = Env::default();
        with_storage(&env, || {
            let custom = RetentionPolicy {
                retain_ledgers: 100,
                archive_before_delete: false,
                honor_holds: true,
                deletion_frozen: false,
            };
            set_retention_policy(&env, DataClass::Telemetry, &custom).unwrap();
            assert_eq!(
                get_retention_policy(&env, DataClass::Telemetry),
                Some(custom.clone())
            );
            assert_eq!(get_effective_policy(&env, DataClass::Telemetry), custom);
            // Invalid overrides are rejected (a zero window would delete instantly).
            let bad = RetentionPolicy {
                retain_ledgers: 0,
                ..RetentionPolicy::default()
            };
            assert_eq!(
                set_retention_policy(&env, DataClass::Telemetry, &bad),
                Err(Error::ConfigInvalid)
            );
        });
    }

    #[test]
    fn plan_reports_before_deleting_and_apply_needs_confirmation() {
        let env = Env::default();
        with_storage(&env, || {
            env.ledger().set_sequence_number(1_000_000);
            let old = record(&env, "tel_old", DataClass::Telemetry, 1, 512);

            let mut records = Vec::new(&env);
            records.push_back(old.clone());
            let plan = plan_cleanup(&env, &records);

            assert!(plan.dry_run);
            assert_eq!(plan.eligible.len(), 1);
            assert_eq!(plan.bytes_reclaimable, 512);
            assert_eq!(plan.protected.len(), 0);
            assert_eq!(plan.retained.len(), 0);

            // Dry run: nothing is destroyed.
            let dry = apply_cleanup(&env, &plan, false).unwrap();
            assert!(!dry.destructive);
            assert_eq!(dry.deleted.len(), 0);
            assert!(get_record(&env, &old.id).is_some());

            // Real run without confirmation is refused.
            let mut live_plan = plan.clone();
            live_plan.dry_run = false;
            assert_eq!(
                apply_cleanup(&env, &live_plan, false),
                Err(Error::InvalidArgument)
            );
            assert!(get_record(&env, &old.id).is_some());

            // Confirmed run deletes and reports.
            let report = apply_cleanup(&env, &live_plan, true).unwrap();
            assert!(report.destructive);
            assert_eq!(report.deleted.len(), 1);
            assert_eq!(report.bytes_reclaimed, 512);
            assert_eq!(report.skipped_protected.len(), 0);
            assert!(get_record(&env, &old.id).is_none());
        });
    }

    #[test]
    fn fresh_records_are_retained_not_eligible() {
        let env = Env::default();
        with_storage(&env, || {
            env.ledger().set_sequence_number(days_to_ledgers(10));
            let recent = record(
                &env,
                "tel_new",
                DataClass::Telemetry,
                days_to_ledgers(9),
                64,
            );
            let mut records = Vec::new(&env);
            records.push_back(recent);
            let plan = plan_cleanup(&env, &records);
            assert_eq!(plan.eligible.len(), 0);
            assert_eq!(plan.retained.len(), 1);
            assert_eq!(plan.bytes_reclaimable, 0);
        });
    }

    #[test]
    fn active_hold_protects_expired_record() {
        let env = Env::default();
        with_storage(&env, || {
            // Past the 365-day default audit window.
            env.ledger().set_sequence_number(days_to_ledgers(400));
            let evidence = record(&env, "audit_1", DataClass::Audit, 1, 2_048);
            place_hold(&env, &evidence.id, HoldReason::Dispute, HOLD_INDEFINITE).unwrap();

            let mut records = Vec::new(&env);
            records.push_back(evidence.clone());
            let plan = plan_cleanup(&env, &records);
            assert_eq!(plan.eligible.len(), 0);
            assert_eq!(plan.protected.len(), 1);
            assert_eq!(plan.bytes_protected, 2_048);

            let mut live_plan = plan.clone();
            live_plan.dry_run = false;
            let report = apply_cleanup(&env, &live_plan, true).unwrap();
            assert_eq!(report.skipped_protected.len(), 1);
            assert!(get_record(&env, &evidence.id).is_some());
        });
    }

    #[test]
    fn hold_placed_after_planning_still_wins() {
        let env = Env::default();
        with_storage(&env, || {
            env.ledger().set_sequence_number(1_000_000);
            let rec = record(&env, "tel_race", DataClass::Telemetry, 1, 128);
            let mut records = Vec::new(&env);
            records.push_back(rec.clone());
            let mut plan = plan_cleanup(&env, &records);
            assert_eq!(plan.eligible.len(), 1);

            // A dispute opens between planning and execution.
            place_hold(&env, &rec.id, HoldReason::Settlement, HOLD_INDEFINITE).unwrap();
            plan.dry_run = false;
            let report = apply_cleanup(&env, &plan, true).unwrap();
            assert_eq!(report.deleted.len(), 0);
            assert_eq!(report.skipped_protected.len(), 1);
            assert!(get_record(&env, &rec.id).is_some());
        });
    }

    #[test]
    fn lapsed_hold_reopens_and_can_be_released() {
        let env = Env::default();
        with_storage(&env, || {
            env.ledger().set_sequence_number(50);
            let rec = record(&env, "tel_hold", DataClass::Telemetry, 1, 32);
            place_hold(&env, &rec.id, HoldReason::Audit, 100).unwrap();
            // Active before the expiry ledger, lapsed at and after it.
            assert!(active_hold(&env, &rec.id, 60).is_some());
            assert!(active_hold(&env, &rec.id, 99).is_some());
            assert!(active_hold(&env, &rec.id, 100).is_none());

            assert_eq!(release_hold(&env, &rec.id), Ok(()));
            assert_eq!(release_hold(&env, &rec.id), Err(Error::NotFound));
            assert!(get_hold(&env, &rec.id).is_none());
        });
    }

    #[test]
    fn frozen_classes_are_never_deleted_even_when_expired() {
        let env = Env::default();
        with_storage(&env, || {
            env.ledger().set_sequence_number(MAX_RETAIN_LEDGERS);
            let settlement = record(&env, "settle_1", DataClass::FinancialSettlement, 1, 4_096);
            let mut records = Vec::new(&env);
            records.push_back(settlement.clone());
            let plan = plan_cleanup(&env, &records);
            assert_eq!(plan.eligible.len(), 0);
            assert_eq!(plan.frozen.len(), 1);
            assert_eq!(plan.bytes_protected, 4_096);

            // Even a hand-forged plan cannot delete a frozen class.
            let forged = CleanupPlan {
                dry_run: false,
                evaluated_ledger: plan.evaluated_ledger,
                eligible: plan.frozen.clone(),
                protected: Vec::new(&env),
                frozen: Vec::new(&env),
                retained: Vec::new(&env),
                bytes_reclaimable: 4_096,
                bytes_protected: 0,
            };
            let report = apply_cleanup(&env, &forged, true).unwrap();
            assert_eq!(report.deleted.len(), 0);
            assert_eq!(report.skipped_protected.len(), 1);
            assert!(get_record(&env, &settlement.id).is_some());
        });
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        let env = Env::default();
        with_storage(&env, || {
            env.ledger().set_sequence_number(1_000);
            let id = Symbol::new(&env, "audit_bad");
            // Holds may not be placed in the past.
            assert_eq!(
                place_hold(&env, &id, HoldReason::Legal, 999),
                Err(Error::ConfigInvalid)
            );
            // An indefinite hold is always allowed.
            assert!(place_hold(&env, &id, HoldReason::Legal, HOLD_INDEFINITE).is_ok());

            let bad_record = RetentionRecord {
                id: id.clone(),
                class: DataClass::Audit,
                created_ledger: 0,
                bytes: 1,
            };
            assert_eq!(put_record(&env, &bad_record), Err(Error::InvalidArgument));
        });
    }
}
