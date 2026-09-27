#![no_std]

pub mod auth;
pub mod batch;
pub mod canonical;
pub mod compat;
pub mod config;
pub mod errors;
pub mod events;
pub mod math;
pub mod migration;
pub mod payments;
pub mod quota;
pub mod retention;
pub mod storage;
pub mod utils;

// Re-export the most commonly-needed items at crate root for ergonomic use.
pub use auth::{get_admin, require_admin, require_not_paused, set_admin};
pub use batch::{
    batch_invoke_no_args, execute_multi_invoke, execute_multi_transfer, multi_transfer_all,
    BatchConfig, BatchError, BatchMode, BatchResult, BatchTransfer, OperationResult,
    ABSOLUTE_MAX_BATCH_SIZE, DEFAULT_MAX_BATCH_SIZE,
};
pub use errors::Error;
pub use migration::{
    begin_migration, clear_journal, dry_run, evaluate_post_checks, expected_step, fail_migration,
    finish_migration, is_resumable, load_journal, mark_step_complete, resume_index, save_journal,
    DryRunReport, MigrationError, MigrationJournal, MigrationPlan, MigrationStatus, MigrationStep,
    MigrationStepKind, PostCheck, PostCheckReport, RollbackStrategy,
};
pub use compat::{
    current_schema_version, downgrade_v2_to_v1, ensure_supported_version, from_latest,
    is_deprecated_version, is_supported_version, migrate_v1_to_v2, new_current_record, to_latest,
    CompatAidStatus, CurrentAidRecord, LegacyAidRecord, VersionedAidRecord,
    CURRENT_RECORD_SCHEMA_VERSION, MAX_SUPPORTED_RECORD_SCHEMA_VERSION,
    MIN_SUPPORTED_RECORD_SCHEMA_VERSION, RECORD_SCHEMA_V1, RECORD_SCHEMA_V2,
};
pub use config::{
    validate_feature_flag, validate_full_config, validate_network_id, validate_rpc_url,
    validate_secret_key, Environment, RedactedSecret,
};
pub use quota::{
    check_and_consume, get_quota_config, get_quota_status, get_usage, reset_quota,
    set_quota_config, QuotaConfig, QuotaStatus, QuotaUsage,
};
pub use retention::{
    active_hold, apply_cleanup, class_label, classify, days_to_ledgers, default_policy,
    get_effective_policy, get_hold, get_record, get_retention_policy, is_frozen, plan_cleanup,
    plan_cleanup_at, place_hold, put_record, release_hold, set_retention_policy, CleanupEntry,
    CleanupPlan, CleanupReport, DataClass, HoldReason, RetentionHold, RetentionPolicy,
    RetentionRecord, HOLD_INDEFINITE, LEDGERS_PER_DAY, MAX_RETAIN_LEDGERS,
};
pub use events::{
    emit, emit_collection_registered, emit_nft_auction, emit_nft_bid, emit_nft_listed,
    emit_nft_offer, emit_nft_settle, emit_nft_sold, emit_royalty_paid, AID_CLAIMED, AID_CREATED,
    AID_REFUNDED, AID_SETTLED, COMMISSION_PAID, CONTRACT_PAUSED, CONTRACT_RESUMED,
    CONTRACT_UPGRADED, PARAMETER_CHANGED, PAYMENT_ESCROW_CREATED, PAYMENT_ESCROW_REFUNDED,
    PAYMENT_ESCROW_RELEASED, PAYMENT_FEE, PAYMENT_TRANSFER, REFERRAL_ACCRUED, REFERRAL_REGISTERED,
    REFERRER_SET, TIER_CONFIG_SET, TREASURY_DEPOSIT, TREASURY_EMERGENCY_WITHDRAW, TREASURY_SET,
    TREASURY_WITHDRAW,
};
pub use payments::{
    calculate_fee, calculate_fee_split, create_escrow, deduct_fee, get_escrow, refund_escrow,
    release_escrow, safe_transfer, safe_transfer_from_contract, EscrowRecord, EscrowState,
    FeeConfig,
};
pub use storage::{
    instance_get, instance_has, instance_remove, instance_set, is_paused, persistent_extend_ttl,
    persistent_get, persistent_has, persistent_remove, persistent_set, set_paused, temporary_get,
    temporary_has, temporary_remove, temporary_set, PERSISTENT_BUMP_AMOUNT,
    PERSISTENT_TTL_THRESHOLD, TEMPORARY_BUMP_AMOUNT, TEMPORARY_TTL_THRESHOLD,
};
pub use utils::{is_expired, now};

pub use canonical::{
    canonical_bytes, canonical_fingerprint, canonicalize_legacy, ensure_supported_encoding,
    is_legacy_encoding, normalize_int, normalize_text, parse_legacy_kv, CanonicalPart,
    CANONICAL_ENCODING_VERSION, LEGACY_ENCODING_VERSION, MAX_FIELD_LEN,
};
#[cfg(test)]
mod test_auth;
#[cfg(test)]
mod test_storage;
