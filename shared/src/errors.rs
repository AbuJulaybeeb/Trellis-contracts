use soroban_sdk::contracterror;

/// Stable error codes shared by every contract in the workspace.
///
/// Codes from 900 to 999 are reserved for errors whose meaning is shared
/// consistently across multiple contract modules.
#[contracterror(export = false)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Caller is not authorised to perform this action.
    Unauthorized = 1,
    /// The requested item was not found in storage.
    NotFound = 2,
    /// The supplied monetary amount is invalid.
    InvalidAmount = 3,
    /// An arithmetic operation would overflow.
    Overflow = 4,
    /// The operation is not permitted while the contract is paused.
    ContractPaused = 5,
    /// The claim link has expired.
    Expired = 6,
    /// The claim has already been used.
    AlreadyClaimed = 7,
    /// Insufficient treasury balance.
    InsufficientBalance = 8,
    /// Requested withdrawal exceeds the configured per-transaction limit.
    WithdrawalLimitExceeded = 9,
    /// A supplied argument is structurally invalid (wrong range, zero amount, etc.).
    InvalidArgument = 10,
    /// The operation requires the contract to be paused but it is currently active.
    NotPaused = 11,
    /// The proposal was not found.
    ProposalNotFound = 12,
    /// The caller has already approved this proposal.
    AlreadyApproved = 13,
    /// The proposal has not reached the approval threshold.
    BelowThreshold = 14,
    /// The proposal has already been executed.
    AlreadyExecuted = 15,
    /// The governance proposal passed its execution window.
    ProposalExpired = 26,
    /// The governance proposal was cancelled by its proposer or super-admin.
    ProposalCancelled = 27,
    /// Attempted to modify an entry that has been marked immutable.
    ImmutableEntry = 16,
    /// The supplied metadata hash is invalid (wrong length or format).
    InvalidHash = 17,
    /// No metadata entry exists for the given identifier.
    MetadataNotFound = 18,
    /// The contract or component was already initialized.
    AlreadyInitialized = 19,
    /// The record uses a schema version this contract does not support.
    UnsupportedSchemaVersion = 20,
    /// A schema migration was attempted but failed validation.
    SchemaMigrationFailed = 21,
    /// The actor has exceeded its quota for this resource/operation.
    QuotaExceeded = 22,
    /// A required configuration value is missing.
    ConfigMissing = 23,
    /// A configuration value is present but malformed or out of range.
    ConfigInvalid = 24,
    /// A secret is unsafe (placeholder, too short, or production-like in local mode).
    UnsafeSecret = 25,

    // ── Upgradeability errors (900–920) ──────────────────────────────────
    /// The target contract is not registered in the upgrade registry.
    ContractNotRegistered = 900,
    /// A contract with the given name is already registered.
    ContractAlreadyRegistered = 901,
    /// The proposed WASM hash matches the current one (no-op upgrade).
    NoChangeDetected = 902,
    /// The upgrade proposal was not found.
    UpgradeProposalNotFound = 903,
    /// The upgrade proposal has already been executed.
    UpgradeAlreadyExecuted = 904,
    /// The migration hook contract call failed.
    MigrationHookFailed = 905,
    /// The contract is already pending an upgrade.
    UpgradeAlreadyPending = 906,
    /// The caller does not hold the Upgrader role.
    NotUpgrader = 907,
    /// The WASM hash is empty or invalid.
    InvalidWasmHash = 908,
    /// Storage layout incompatibility detected during migration.
    StorageIncompatible = 909,
    /// The migration hook address is not a valid contract.
    InvalidMigrationHook = 910,

    // ── Payment errors (700–720) ──────────────────────────────────────
    /// The payment amount is not strictly positive.
    PaymentInvalidAmount = 700,
    /// The sender has insufficient token balance for this transfer.
    PaymentInsufficientBalance = 701,
    /// The escrow deposit does not exist or has already been released.
    PaymentEscrowNotFound = 702,
    /// The escrow deposit has already been released to the beneficiary.
    PaymentEscrowAlreadyReleased = 703,
    /// The escrow deposit has already been refunded to the depositor.
    PaymentEscrowAlreadyRefunded = 704,
    /// The caller is not authorised to release or refund this escrow.
    PaymentEscrowUnauthorized = 705,
    /// The escrow deposit has expired and cannot be released.
    PaymentEscrowExpired = 706,
    /// The escrow deposit has not yet expired and cannot be refunded.
    PaymentEscrowNotExpired = 707,
    /// The fee basis-point rate is out of the valid 0–10 000 range.
    PaymentInvalidFeeRate = 708,
    /// The fee calculation resulted in an arithmetic overflow.
    PaymentFeeOverflow = 709,
    /// The escrow ID counter has overflowed.
    PaymentEscrowIdOverflow = 710,

    // ── NFT Marketplace errors (1000–1008) ──────────────────────────────
    /// The NFT listing was not found.
    ListingNotFound = 1000,
    /// The NFT listing has already been sold or cancelled.
    ListingAlreadySold = 1001,
    /// The caller is not the owner of this NFT.
    NotOwner = 1002,
    /// The collection is already registered.
    CollectionAlreadyRegistered = 1003,
    /// The collection was not found.
    CollectionNotFound = 1004,
    /// The currency address is not whitelisted.
    CurrencyNotWhitelisted = 1005,
    /// The metadata hash is invalid (wrong length).
    InvalidMetadataHash = 1006,
    /// The royalty basis-point rate is out of range.
    InvalidRoyaltyRate = 1007,
    /// The auto-extension window is invalid.
    InvalidExtensionWindow = 1008,
    /// Illegal state transition.
    InvalidTransition = 950,
    /// Aid not yet expired.
    AidNotExpiredYet = 951,
    /// Aid already refunded.
    AidAlreadyRefunded = 952,
}
