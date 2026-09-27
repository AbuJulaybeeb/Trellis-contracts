//! Migration safety framework (Issue #51).
//!
//! Schema and data migrations are the riskiest operations a contract can run:
//! they touch existing ledger state, and a partial rollout can leave the
//! contract inconsistent with its code. This module adds guardrails that let
//! maintainers **preview** a migration, **validate** its outcome, and
//! **recover** deterministically when a step fails.
//!
//! ## Components
//!
//! - [`MigrationPlan`] — ordered, versioned list of migration steps plus the
//!   rollback strategy and operator notes. [`dry_run`] reports the impact
//!   (step count, affected records, destructive steps) **before any write**.
//! - [`MigrationJournal`] — persisted progress record. Steps are marked
//!   complete strictly in order, so a resumed migration always continues from
//!   the same deterministic point ([`resume_index`]).
//! - [`evaluate_post_checks`] / [`finish_migration`] — post-migration
//!   validation. Any failing check fails the migration and records the first
//!   failed check id, so operators know exactly what to forward-fix.
//!
//! ## Safe defaults
//!
//! A missing journal means "no migration in progress". [`dry_run`] never writes
//! to storage and is safe to expose on a read path.
//!
//! See `docs/MIGRATION_SAFETY.md` for the operator runbook and rollback notes.

use soroban_sdk::{contracterror, contracttype, symbol_short, Env, String, Symbol, Vec};

use crate::storage::{instance_get, instance_remove, instance_set};

/// Journal storage key (instance storage — one active migration per contract).
const JOURNAL_KEY: Symbol = symbol_short!("mig_jrnl");

/// Migration orchestration errors (range 920–939).
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MigrationError {
    /// The plan is malformed (bad version range, empty, or unordered steps).
    InvalidPlan = 920,
    /// A migration is already in progress; finish, fail, or clear it first.
    AlreadyInProgress = 921,
    /// The journal exists but is not in an in-progress state.
    NotInProgress = 922,
    /// A step was completed out of order or does not match the plan.
    StepOutOfOrder = 923,
    /// One or more post-migration checks failed.
    PostCheckFailed = 924,
    /// No journal exists for this contract.
    NoJournal = 925,
}

// ---------------------------------------------------------------------------
// Plan types
// ---------------------------------------------------------------------------

/// What a migration step touches. Used by [`dry_run`] to describe impact.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationStepKind {
    /// Re-encodes an existing storage entry into a new layout.
    Storage,
    /// Changes the schema/version envelope of stored records.
    Schema,
    /// Backfills records that did not exist in the old version.
    DataBackfill,
    /// Changes contract configuration values.
    Config,
    /// Anything else the operator wants to describe explicitly.
    Custom,
}

/// How to recover if the migration fails after it has started.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RollbackStrategy {
    /// Re-deploy the previous WASM / restore the pre-migration snapshot.
    Revert,
    /// Ship a corrective forward migration; do not roll state back.
    ForwardFix,
    /// Requires manual intervention; escalate to the ops runbook.
    Manual,
}

/// A single ordered migration step.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationStep {
    /// Strictly increasing step id; defines the deterministic execution order.
    pub id: u32,
    /// Short human-readable step name (e.g. `"remap_keys"`).
    pub name: Symbol,
    /// Category of work performed by this step.
    pub kind: MigrationStepKind,
    /// Estimated number of stored records touched when this step runs.
    pub affected_records: u32,
    /// `true` when the step deletes or overwrites data (needs approval).
    pub destructive: bool,
}

/// An ordered migration plan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationPlan {
    /// Schema version the contract currently stores.
    pub from_version: u32,
    /// Schema version this plan migrates to (must be greater than `from`).
    pub to_version: u32,
    /// Steps in execution order (strictly increasing ids).
    pub steps: Vec<MigrationStep>,
    /// Recovery strategy if the migration fails midway.
    pub rollback: RollbackStrategy,
    /// Operator-facing rollback / forward-fix notes.
    pub rollback_notes: String,
}

impl MigrationPlan {
    /// Validate version range, non-empty steps, and deterministic ordering.
    pub fn validate(&self) -> Result<(), MigrationError> {
        if self.to_version <= self.from_version {
            return Err(MigrationError::InvalidPlan);
        }
        if self.steps.is_empty() {
            return Err(MigrationError::InvalidPlan);
        }
        let mut prev: u32 = 0;
        for step in self.steps.iter() {
            if step.id <= prev {
                return Err(MigrationError::InvalidPlan);
            }
            prev = step.id;
        }
        Ok(())
    }

    /// Sum of `affected_records` across every step (saturating).
    pub fn total_affected_records(&self) -> u32 {
        let mut total: u32 = 0;
        for step in self.steps.iter() {
            total = total.saturating_add(step.affected_records);
        }
        total
    }

    /// Number of destructive steps in the plan.
    pub fn destructive_steps(&self) -> u32 {
        let mut total: u32 = 0;
        for step in self.steps.iter() {
            if step.destructive {
                total = total.saturating_add(1);
            }
        }
        total
    }

    /// Destructive migrations require an explicit approval step before running.
    pub fn requires_approval(&self) -> bool {
        self.destructive_steps() > 0
    }

    /// Operator-facing rollback note recorded with the plan.
    pub fn rollback_note(&self) -> String {
        self.rollback_notes.clone()
    }
}

// ---------------------------------------------------------------------------
// Dry run
// ---------------------------------------------------------------------------

/// Impact report produced by [`dry_run`] without touching storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DryRunReport {
    /// Whether the plan is well formed and safe to stage.
    pub plan_valid: bool,
    /// 0 when valid, otherwise the [`MigrationError`] code that failed.
    pub error_code: u32,
    /// Number of steps in the plan.
    pub step_count: u32,
    /// Total estimated records touched across all steps (0 when invalid).
    pub affected_records: u32,
    /// Number of destructive steps (0 when invalid).
    pub destructive_steps: u32,
    /// `true` when a destructive step means approval is required first.
    pub requires_approval: bool,
}

/// Preview a migration's impact **before any writes happen**.
///
/// This is a pure function: it never reads or writes contract storage, so it
/// can be called on a read-only path and in simulations.
pub fn dry_run(plan: &MigrationPlan) -> DryRunReport {
    match plan.validate() {
        Ok(()) => DryRunReport {
            plan_valid: true,
            error_code: 0,
            step_count: plan.steps.len(),
            affected_records: plan.total_affected_records(),
            destructive_steps: plan.destructive_steps(),
            requires_approval: plan.requires_approval(),
        },
        Err(e) => DryRunReport {
            plan_valid: false,
            error_code: e as u32,
            step_count: plan.steps.len(),
            affected_records: 0,
            destructive_steps: 0,
            requires_approval: true,
        },
    }
}

// ---------------------------------------------------------------------------
// Post checks
// ---------------------------------------------------------------------------

/// A single post-migration validation check.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCheck {
    /// Stable check id (e.g. `"row_count"`).
    pub id: Symbol,
    /// Result reported by the operator's validation script.
    pub passed: bool,
}

/// Aggregated post-check outcome.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PostCheckReport {
    /// Total checks evaluated.
    pub total: u32,
    /// Checks that passed.
    pub passed: u32,
    /// Checks that failed.
    pub failed: u32,
    /// `true` when every check passed.
    pub ok: bool,
    /// First failing check id, for actionable forward-fix guidance.
    pub first_failed: Option<Symbol>,
}

/// Evaluate post-migration checks. Pure function — no storage access.
pub fn evaluate_post_checks(checks: &Vec<PostCheck>) -> PostCheckReport {
    let mut passed: u32 = 0;
    let mut failed: u32 = 0;
    let mut first_failed: Option<Symbol> = None;
    for check in checks.iter() {
        if check.passed {
            passed = passed.saturating_add(1);
        } else {
            failed = failed.saturating_add(1);
            if first_failed.is_none() {
                first_failed = Some(check.id.clone());
            }
        }
    }
    PostCheckReport {
        total: checks.len(),
        passed,
        failed,
        ok: failed == 0,
        first_failed,
    }
}

// ---------------------------------------------------------------------------
// Journal
// ---------------------------------------------------------------------------

/// Lifecycle state of the persisted migration journal.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationStatus {
    /// No migration has started.
    NotStarted,
    /// A migration is currently running.
    InProgress,
    /// Every step completed and all post-checks passed.
    Completed,
    /// The migration aborted; the journal keeps the failure point.
    Failed,
}

/// Persisted progress record for the active migration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationJournal {
    /// Current lifecycle state.
    pub status: MigrationStatus,
    /// Source schema version recorded when the migration started.
    pub from_version: u32,
    /// Target schema version recorded when the migration started.
    pub to_version: u32,
    /// Number of steps completed so far (also the next step index).
    pub completed_steps: u32,
    /// Total steps in the plan.
    pub total_steps: u32,
    /// Ledger timestamp when the migration started.
    pub started_at: u64,
    /// Ledger timestamp of the last journal update.
    pub updated_at: u64,
    /// Name of the step that failed, when `status == Failed`.
    pub failed_step: Option<Symbol>,
    /// Error code that caused the failure (0 when none).
    pub last_error: u32,
}

/// Load the active migration journal, if one exists.
pub fn load_journal(env: &Env) -> Option<MigrationJournal> {
    instance_get(env, &JOURNAL_KEY)
}

/// Persist the migration journal.
pub fn save_journal(env: &Env, journal: &MigrationJournal) {
    instance_set(env, &JOURNAL_KEY, journal);
}

/// Remove the journal. Operators call this only after a completed migration
/// has been recorded elsewhere, or after abandoning a failed attempt.
pub fn clear_journal(env: &Env) {
    instance_remove(env, &JOURNAL_KEY);
}

/// Start a migration. Fails if a different migration is already in progress.
pub fn begin_migration(
    env: &Env,
    plan: &MigrationPlan,
    at: u64,
) -> Result<MigrationJournal, MigrationError> {
    plan.validate()?;
    if let Some(existing) = load_journal(env) {
        if existing.status == MigrationStatus::InProgress {
            return Err(MigrationError::AlreadyInProgress);
        }
    }
    let journal = MigrationJournal {
        status: MigrationStatus::InProgress,
        from_version: plan.from_version,
        to_version: plan.to_version,
        completed_steps: 0,
        total_steps: plan.steps.len(),
        started_at: at,
        updated_at: at,
        failed_step: None,
        last_error: 0,
    };
    save_journal(env, &journal);
    Ok(journal)
}

/// The step that must run next for the given journal position.
pub fn expected_step(plan: &MigrationPlan, journal: &MigrationJournal) -> Option<MigrationStep> {
    plan.steps.get(journal.completed_steps)
}

/// Mark the next step complete. Steps must be completed in plan order so a
/// resumed migration is always deterministic.
pub fn mark_step_complete(
    env: &Env,
    plan: &MigrationPlan,
    step_id: u32,
    at: u64,
) -> Result<MigrationJournal, MigrationError> {
    let mut journal = load_journal(env).ok_or(MigrationError::NoJournal)?;
    if journal.status != MigrationStatus::InProgress {
        return Err(MigrationError::NotInProgress);
    }
    let expected = plan
        .steps
        .get(journal.completed_steps)
        .ok_or(MigrationError::StepOutOfOrder)?;
    if expected.id != step_id {
        return Err(MigrationError::StepOutOfOrder);
    }
    journal.completed_steps = journal.completed_steps.saturating_add(1);
    journal.updated_at = at;
    save_journal(env, &journal);
    Ok(journal)
}

/// Record a failure at a step so the migration can be resumed or rolled back.
pub fn fail_migration(
    env: &Env,
    failed_step: &Symbol,
    error_code: u32,
    at: u64,
) -> Result<MigrationJournal, MigrationError> {
    let mut journal = load_journal(env).ok_or(MigrationError::NoJournal)?;
    journal.status = MigrationStatus::Failed;
    journal.failed_step = Some(failed_step.clone());
    journal.last_error = error_code;
    journal.updated_at = at;
    save_journal(env, &journal);
    Ok(journal)
}

/// Finish a migration after running post-checks.
///
/// All checks must pass. A failing check marks the journal `Failed` and
/// returns [`MigrationError::PostCheckFailed`] so the caller can follow the
/// plan's rollback strategy.
pub fn finish_migration(
    env: &Env,
    checks: &Vec<PostCheck>,
    at: u64,
) -> Result<MigrationJournal, MigrationError> {
    let mut journal = load_journal(env).ok_or(MigrationError::NoJournal)?;
    if journal.status != MigrationStatus::InProgress {
        return Err(MigrationError::NotInProgress);
    }
    let report = evaluate_post_checks(checks);
    if !report.ok {
        journal.status = MigrationStatus::Failed;
        journal.failed_step = report.first_failed.clone();
        journal.last_error = MigrationError::PostCheckFailed as u32;
        journal.updated_at = at;
        save_journal(env, &journal);
        return Err(MigrationError::PostCheckFailed);
    }
    journal.status = MigrationStatus::Completed;
    journal.completed_steps = journal.total_steps;
    journal.updated_at = at;
    save_journal(env, &journal);
    Ok(journal)
}

/// Index of the next step to run — the deterministic resume point.
pub fn resume_index(journal: &MigrationJournal) -> u32 {
    journal.completed_steps
}

/// Whether a journal can still be resumed (in progress or failed).
pub fn is_resumable(journal: &MigrationJournal) -> bool {
    journal.status == MigrationStatus::InProgress || journal.status == MigrationStatus::Failed
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct DummyContract;
    #[contractimpl]
    impl DummyContract {}

    fn step(env: &Env, id: u32, name: &str, records: u32, destructive: bool) -> MigrationStep {
        MigrationStep {
            id,
            name: Symbol::new(env, name),
            kind: MigrationStepKind::Schema,
            affected_records: records,
            destructive,
        }
    }

    fn plan(env: &Env) -> MigrationPlan {
        let mut steps = Vec::new(env);
        steps.push_back(step(env, 1, "remap", 120, false));
        steps.push_back(step(env, 2, "prune", 5, true));
        MigrationPlan {
            from_version: 1,
            to_version: 2,
            steps,
            rollback: RollbackStrategy::ForwardFix,
            rollback_notes: String::from_str(env, "forward-fix only; keep old keys"),
        }
    }

    #[test]
    fn dry_run_reports_impact_without_writing() {
        let env = Env::default();
        let contract = env.register_contract(None, DummyContract);
        env.as_contract(&contract, || {
            let p = plan(&env);
            let report = dry_run(&p);
            assert!(report.plan_valid);
            assert_eq!(report.error_code, 0);
            assert_eq!(report.step_count, 2);
            assert_eq!(report.affected_records, 125);
            assert_eq!(report.destructive_steps, 1);
            assert!(report.requires_approval);
            // Dry run must not create a journal.
            assert!(load_journal(&env).is_none());
        });
    }

    #[test]
    fn invalid_plan_is_reported_not_panicked() {
        let env = Env::default();
        let mut steps = Vec::new(&env);
        steps.push_back(step(&env, 1, "a", 1, false));
        let bad = MigrationPlan {
            from_version: 3,
            to_version: 3,
            steps,
            rollback: RollbackStrategy::Manual,
            rollback_notes: String::from_str(&env, "n/a"),
        };
        let report = dry_run(&bad);
        assert!(!report.plan_valid);
        assert_eq!(report.error_code, MigrationError::InvalidPlan as u32);
        assert_eq!(bad.validate(), Err(MigrationError::InvalidPlan));

        // Unordered step ids are also rejected.
        let mut unordered = Vec::new(&env);
        unordered.push_back(step(&env, 2, "b", 1, false));
        unordered.push_back(step(&env, 1, "c", 1, false));
        let bad2 = MigrationPlan {
            from_version: 1,
            to_version: 2,
            steps: unordered,
            rollback: RollbackStrategy::Manual,
            rollback_notes: String::from_str(&env, "n/a"),
        };
        assert_eq!(bad2.validate(), Err(MigrationError::InvalidPlan));
    }

    #[test]
    fn journal_advances_in_order_and_resumes() {
        let env = Env::default();
        let contract = env.register_contract(None, DummyContract);
        env.as_contract(&contract, || {
            let p = plan(&env);
            let mut journal = begin_migration(&env, &p, 10).unwrap();
            assert_eq!(journal.status, MigrationStatus::InProgress);

            // Wrong / out-of-order step id is rejected.
            assert_eq!(
                mark_step_complete(&env, &p, 2, 11),
                Err(MigrationError::StepOutOfOrder)
            );
            journal = mark_step_complete(&env, &p, 1, 12).unwrap();
            assert_eq!(resume_index(&journal), 1);
            assert!(is_resumable(&journal));
            // The next expected step is the one at the resume index.
            assert_eq!(expected_step(&p, &journal).unwrap().id, 2);

            journal = mark_step_complete(&env, &p, 2, 13).unwrap();
            assert_eq!(resume_index(&journal), 2);

            let mut checks = Vec::new(&env);
            checks.push_back(PostCheck {
                id: symbol_short!("rows"),
                passed: true,
            });
            let finished = finish_migration(&env, &checks, 14).unwrap();
            assert_eq!(finished.status, MigrationStatus::Completed);
            assert_eq!(finished.completed_steps, 2);
            assert!(!is_resumable(&finished));
        });
    }

    #[test]
    fn failed_post_check_marks_migration_failed() {
        let env = Env::default();
        let contract = env.register_contract(None, DummyContract);
        env.as_contract(&contract, || {
            let p = plan(&env);
            begin_migration(&env, &p, 1).unwrap();
            mark_step_complete(&env, &p, 1, 2).unwrap();
            mark_step_complete(&env, &p, 2, 3).unwrap();

            let mut checks = Vec::new(&env);
            checks.push_back(PostCheck {
                id: symbol_short!("rows"),
                passed: true,
            });
            checks.push_back(PostCheck {
                id: symbol_short!("sums"),
                passed: false,
            });
            assert_eq!(
                finish_migration(&env, &checks, 4),
                Err(MigrationError::PostCheckFailed)
            );
            let journal = load_journal(&env).unwrap();
            assert_eq!(journal.status, MigrationStatus::Failed);
            assert_eq!(journal.last_error, MigrationError::PostCheckFailed as u32);
            assert_eq!(journal.failed_step, Some(symbol_short!("sums")));
        });
    }

    #[test]
    fn cannot_start_twice_while_in_progress_and_can_clear() {
        let env = Env::default();
        let contract = env.register_contract(None, DummyContract);
        env.as_contract(&contract, || {
            let p = plan(&env);
            begin_migration(&env, &p, 1).unwrap();
            assert_eq!(
                begin_migration(&env, &p, 2),
                Err(MigrationError::AlreadyInProgress)
            );

            // Failing a step records the failure and frees the lock once cleared.
            fail_migration(&env, &symbol_short!("remap"), 7, 3).unwrap();
            let failed = load_journal(&env).unwrap();
            assert_eq!(failed.status, MigrationStatus::Failed);
            assert_eq!(failed.last_error, 7);
            assert!(is_resumable(&failed));

            clear_journal(&env);
            assert!(load_journal(&env).is_none());
            assert!(begin_migration(&env, &p, 4).is_ok());
        });
    }

    #[test]
    fn evaluate_post_checks_reports_first_failure() {
        let env = Env::default();
        let mut checks = Vec::new(&env);
        checks.push_back(PostCheck { id: symbol_short!("a"), passed: true });
        checks.push_back(PostCheck { id: symbol_short!("b"), passed: false });
        checks.push_back(PostCheck { id: symbol_short!("c"), passed: false });
        let report = evaluate_post_checks(&checks);
        assert_eq!(report.total, 3);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 2);
        assert!(!report.ok);
        assert_eq!(report.first_failed, Some(symbol_short!("b")));
    }
}
