use soroban_sdk::{contracttype, Address, Vec};

/// Contract configuration stored in instance storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub treasury: Address,
    pub token: Address,
    pub default_expiry_secs: u64,
}

/// Lifecycle state of an aid record.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AidStatus {
    /// Escrowed, waiting for a valid claim.
    Pending,
    /// Claimed by the recipient; funds transferred out.
    Settled,
    /// Expired and refunded back to the donor (never claimed).
    Refunded,
}

/// A single aid disbursement record, escrowed inside this contract until
/// claimed by the recipient (or refunded to the donor after expiry).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AidRecord {
    pub id: u64,
    pub donor: Address,
    pub recipient: Address,
    pub token: Address,
    pub amount: i128,
    /// Ledger sequence after which the aid can no longer be claimed.
    pub expiry_ledger: u32,
    pub status: AidStatus,
}

/// One page of results from a paginated aid query.
///
/// Pass `next_cursor` back as the `cursor` argument to fetch the following
/// page; `None` means the result set is exhausted.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AidPage {
    pub records: Vec<AidRecord>,
    pub next_cursor: Option<u32>,
}

/// Outcome of rebuilding the discovery index from canonical aid storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchIndexRepairReport {
    /// Number of pending, visible aid records placed in the rebuilt index.
    pub indexed: u32,
    /// Entries missing from the previous index.
    pub added: u32,
    /// Stale entries removed from the previous index.
    pub removed: u32,
}
