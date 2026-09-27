//! Aid-contract storage layer.
//!
//! All persistent reads and writes are routed through the shared storage
//! helpers so that TTL bumps are applied consistently on every access.
//!
//! ## Key layout
//!
//! | Key                    | Storage type | Rationale                                      |
//! |------------------------|--------------|------------------------------------------------|
//! | `Aid(id)`              | persistent   | Long-lived financial record; must outlive many |
//! |                        |              | ledger closures until claimed/refunded.        |
//! | `AidCounter`           | instance     | Config-level counter; always needed when live. |
//! | `Token`                | instance     | Escrow token configured at initialisation.     |
//! | `DonorIndex(donor)`    | persistent   | Append-only aid-ID list per donor; grows with  |
//! |                        |              | the donor's history and must persist forever.  |
//! | `RecipientIndex(addr)` | persistent   | Append-only aid-ID list per recipient.         |

use soroban_sdk::{contracttype, Address, Env, Vec};

use shared::storage::{
    instance_get, instance_set, persistent_get, persistent_has, persistent_remove, persistent_set,
};

use crate::types::AidRecord;

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

/// Keys used by the aid contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Persistent record for a single aid disbursement.
    Aid(u64),
    /// Monotonically-increasing aid identifier counter (instance).
    AidCounter,
    /// Token address escrowed by this contract (instance).
    Token,
    /// Treasury address (instance).
    Treasury,
    /// Default expiry in seconds (instance).
    DefaultExpiry,
    /// Configuration marker (instance) - set on initialize.
    Initialized,
    /// Append-only list of aid IDs created by a donor (persistent).
    DonorAids(Address),
    /// Append-only list of aid IDs assigned to a recipient (persistent).
    RecipientIndex(Address),
    /// IDs currently eligible for discovery search. Canonical records remain
    /// the source of truth; this is a repairable derived index.
    SearchIndex,
    /// An additional address permitted to discover a particular aid record.
    SearchAccess(u64, Address),
    /// Admin-controlled discovery visibility flag. Missing means visible for
    /// backwards-compatible records.
    SearchHidden(u64),
}

// ---------------------------------------------------------------------------
// Aid record helpers — persistent storage with automatic TTL extension
// ---------------------------------------------------------------------------

/// Read an aid record from persistent storage.
///
/// Returns `None` when the ID does not exist.  Extends the entry TTL on every
/// hit so frequently-accessed records are never evicted while active.
#[inline]
pub fn get_aid(env: &Env, aid_id: u64) -> Option<AidRecord> {
    persistent_get(env, &DataKey::Aid(aid_id))
}

/// Write (or overwrite) an aid record to persistent storage.
///
/// TTL is extended immediately so the entry survives upcoming ledger closures.
#[inline]
pub fn set_aid(env: &Env, aid_id: u64, record: &AidRecord) {
    persistent_set(env, &DataKey::Aid(aid_id), record);
}

/// Returns `true` when an aid record with the given ID exists.
///
/// Does **not** extend TTL — call [`get_aid`] when you need the value.
#[inline]
pub fn has_aid(env: &Env, aid_id: u64) -> bool {
    persistent_has(env, &DataKey::Aid(aid_id))
}

/// Remove an aid record from persistent storage (e.g. after full settlement).
#[inline]
pub fn remove_aid(env: &Env, aid_id: u64) {
    persistent_remove(env, &DataKey::Aid(aid_id));
}

// ---------------------------------------------------------------------------
// Aid counter — instance storage
// ---------------------------------------------------------------------------

/// Read the current aid counter, defaulting to 0 if never set.
#[inline]
pub fn get_aid_counter(env: &Env) -> u64 {
    instance_get(env, &DataKey::AidCounter).unwrap_or(0)
}

/// Write the aid counter to instance storage.
#[inline]
pub fn set_aid_counter(env: &Env, counter: u64) {
    instance_set(env, &DataKey::AidCounter, &counter);
}

// ---------------------------------------------------------------------------
// Escrow token — instance storage
// ---------------------------------------------------------------------------

/// Read the configured escrow token, if initialised.
pub fn get_token(env: &Env) -> Option<Address> {
    instance_get(env, &DataKey::Token)
}

/// Store the escrow token address (initialisation only).
pub fn set_token(env: &Env, token: &Address) {
    instance_set(env, &DataKey::Token, token);
}

// ---------------------------------------------------------------------------
// Participant indexes — append-only ID lists for pagination
// ---------------------------------------------------------------------------

/// Read the full list of aid IDs created by `donor`.
///
/// The list is append-only: entries are added on `create_aid` and never
/// removed, so claim/refund transitions only mutate the underlying records,
/// keeping every index entry valid.
pub fn get_donor_aids(env: &Env, donor: &Address) -> Vec<u64> {
    persistent_get(env, &DataKey::DonorAids(donor.clone())).unwrap_or_else(|| Vec::new(env))
}

/// Append `aid_id` to `donor`'s index.
pub fn append_donor_aid(env: &Env, donor: &Address, aid_id: u64) {
    let mut ids = get_donor_aids(env, donor);
    ids.push_back(aid_id);
    persistent_set(env, &DataKey::DonorAids(donor.clone()), &ids);
}

/// Read the full list of aid IDs assigned to `recipient`.
pub fn get_recipient_aids(env: &Env, recipient: &Address) -> Vec<u64> {
    persistent_get(env, &DataKey::RecipientAids(recipient.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Append `aid_id` to `recipient`'s index.
pub fn append_recipient_aid(env: &Env, recipient: &Address, aid_id: u64) {
    let mut ids = get_recipient_aids(env, recipient);
    ids.push_back(aid_id);
    persistent_set(env, &DataKey::RecipientAids(recipient.clone()), &ids);
}

pub fn get_donor_index(env: &Env, donor: &Address) -> Vec<u64> {
    get_donor_aids(env, donor)
}

pub fn append_donor_index(env: &Env, donor: &Address, aid_id: u64) {
    append_donor_aid(env, donor, aid_id);
}

pub fn get_recipient_index(env: &Env, recipient: &Address) -> Vec<u64> {
    get_recipient_aids(env, recipient)
}

pub fn append_recipient_index(env: &Env, recipient: &Address, aid_id: u64) {
    append_recipient_aid(env, recipient, aid_id);
}

// ---------------------------------------------------------------------------
// Permission-aware discovery index
// ---------------------------------------------------------------------------

pub fn get_search_index(env: &Env) -> Vec<u64> {
    persistent_get(env, &DataKey::SearchIndex).unwrap_or_else(|| Vec::new(env))
}

pub fn set_search_index(env: &Env, ids: &Vec<u64>) {
    persistent_set(env, &DataKey::SearchIndex, ids);
}

pub fn is_search_hidden(env: &Env, aid_id: u64) -> bool {
    persistent_get(env, &DataKey::SearchHidden(aid_id)).unwrap_or(false)
}

pub fn set_search_hidden(env: &Env, aid_id: u64, hidden: bool) {
    persistent_set(env, &DataKey::SearchHidden(aid_id), &hidden);
}

pub fn has_search_access(env: &Env, aid_id: u64, viewer: &Address) -> bool {
    persistent_get(env, &DataKey::SearchAccess(aid_id, viewer.clone())).unwrap_or(false)
}

pub fn set_search_access(env: &Env, aid_id: u64, viewer: &Address, granted: bool) {
    persistent_set(
        env,
        &DataKey::SearchAccess(aid_id, viewer.clone()),
        &granted,
    );
}

// ---------------------------------------------------------------------------
// Treasury address — instance storage
// ---------------------------------------------------------------------------

/// Read the configured treasury address, if initialised.
pub fn get_treasury(env: &Env) -> Option<Address> {
    instance_get(env, &DataKey::Treasury)
}

/// Store the treasury address (initialisation only).
pub fn set_treasury(env: &Env, treasury: &Address) {
    instance_set(env, &DataKey::Treasury, treasury);
}

// ---------------------------------------------------------------------------
// Default expiry — instance storage
// ---------------------------------------------------------------------------

/// Read the default expiry in seconds, if set.
pub fn get_default_expiry(env: &Env) -> Option<u64> {
    instance_get(env, &DataKey::DefaultExpiry)
}

/// Store the default expiry in seconds.
pub fn set_default_expiry(env: &Env, expiry_secs: u64) {
    instance_set(env, &DataKey::DefaultExpiry, &expiry_secs);
}

// ---------------------------------------------------------------------------
// Initialization flag — instance storage
// ---------------------------------------------------------------------------

/// Check if the contract has been initialized.
pub fn is_initialized(env: &Env) -> bool {
    instance_get::<DataKey, bool>(env, &DataKey::Initialized).unwrap_or(false)
}

/// Mark the contract as initialized.
pub fn set_initialized(env: &Env) {
    instance_set(env, &DataKey::Initialized, &true);
}
