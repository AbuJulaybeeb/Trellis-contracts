//! User-facing activity timeline with privacy-aware event filtering.
//!
//! Trellis contracts already emit rich ledger events, but ledger events are
//! written for *integrators*, not for the people whose records they describe.
//! A user asking "what happened to my record?" needs a filtered, ordered,
//! paginated view — and must never see maintainer-only operational context.
//!
//! This module provides that view.
//!
//! ## Two separate stores
//!
//! | Store              | Written by            | Read by                     |
//! |--------------------|-----------------------|-----------------------------|
//! | User timeline      | [`append_user_event`] | [`timeline_page`]           |
//! | Maintainer audit   | [`record_audit_event`]| [`audit_trail`]             |
//!
//! The stores live under different storage keys and have different accessors,
//! so a maintainer-only entry is structurally incapable of appearing in a
//! user-facing page: there is no code path from [`TimelineKey::AuditEntry`] to
//! [`timeline_page`]. [`append_user_event`] also refuses
//! [`Visibility::Maintainer`] outright, so the separation cannot be broken by
//! passing the wrong flag either.
//!
//! ## Visibility rules
//!
//! | [`Visibility`]     | Anonymous | Participant | Maintainer |
//! |--------------------|-----------|-------------|------------|
//! | `Public`           | yes       | yes         | yes        |
//! | `Participant`      | no        | yes         | yes        |
//! | `Maintainer`       | no        | no          | audit only |
//!
//! "Participant" means the viewer is the entry's `actor` or its `subject`.
//! "Maintainer" means the viewer holds [`Role::Admin`] or [`Role::Upgrader`] —
//! see [`is_maintainer`].
//!
//! ## Ordering and pagination
//!
//! Every entry gets a monotonically increasing `seq` from an append-only
//! counter. Pages are always returned in `seq` order and are addressed by the
//! `seq` of the last entry the caller saw, so:
//!
//! - redacting or deleting an entry never renumbers or reorders anything else;
//! - a page never repeats or skips an entry that was visible when the cursor
//!   was issued;
//! - concurrent appends can only extend the sequence, never shift it.
//!
//! A single request examines at most [`MAX_SCAN_PER_PAGE`] sequence numbers, so
//! a page whose visibility filter rejects most entries returns a short page
//! together with a `next_cursor` the caller can resume from. That keeps a read
//! bounded no matter how sparse the visible entries become.

use soroban_sdk::{contracttype, symbol_short, Address, Bytes, BytesN, Env, Symbol, Vec};

use crate::auth::{has_role, require_role, Role};
use crate::canonical::{canonical_fingerprint, CanonicalPart};
use crate::errors::Error;
use crate::storage::{persistent_get, persistent_has, persistent_remove, persistent_set};

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

/// Page size used when a caller passes `0`.
pub const DEFAULT_PAGE_SIZE: u32 = 20;

/// Largest page a caller may request. Bounds the work done by one read.
pub const MAX_PAGE_SIZE: u32 = 50;

/// Largest number of sequence numbers examined while filling one page.
///
/// Sparse visibility (many maintainer-only or deleted entries) makes a request
/// return a shorter page rather than scanning unbounded storage.
pub const MAX_SCAN_PER_PAGE: u64 = 256;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Who is allowed to see a timeline entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Visibility {
    /// Readable by anyone, including anonymous callers.
    Public,
    /// Readable by the entry's `actor` or `subject`, and by maintainers.
    Participant,
    /// Maintainer-only operational context. Stored in the audit store and
    /// never returned by [`timeline_page`].
    Maintainer,
}

/// The kinds of activity a user-facing timeline can carry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimelineEventType {
    /// A record was created.
    RecordCreated,
    /// A record's mutable fields changed.
    RecordUpdated,
    /// A record reached its terminal successful state.
    RecordSettled,
    /// A record passed its deadline without being settled.
    RecordExpired,
    /// Access to a record was narrowed (hold, freeze, restriction).
    RecordRestricted,
    /// Funds left the contract towards a user.
    PaymentSent,
    /// Funds returned to a user.
    PaymentRefunded,
    /// A role grant or revocation.
    RoleChanged,
    /// A configuration value changed.
    ConfigChanged,
    /// The contract was paused.
    ContractPaused,
    /// The contract resumed.
    ContractResumed,
}

/// A stable pointer at the resource an entry describes.
///
/// `revision` is the resource's own revision counter. Bumping it produces a new
/// [`ResourceLink::anchor`], so a link captured in a UI can never silently
/// resolve to content that has since changed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceLink {
    /// Resource family, e.g. `Bytes::from_slice(env, b"aid")`.
    pub kind: Bytes,
    /// The resource's primary key.
    pub id: u64,
    /// The resource's revision at the time of the event.
    pub revision: u32,
}

impl ResourceLink {
    /// Deterministic 32-byte anchor for this link.
    ///
    /// Uses the workspace canonical encoding, so the same resource and revision
    /// always produce the same anchor and every part is length-delimited.
    pub fn anchor(&self, env: &Env) -> Result<BytesN<32>, Error> {
        let mut parts = Vec::new(env);
        parts.push_back(CanonicalPart::Text(self.kind.clone()));
        parts.push_back(CanonicalPart::Int(i128::from(self.id)));
        parts.push_back(CanonicalPart::Int(i128::from(self.revision)));
        canonical_fingerprint(env, &parts)
    }

    /// Returns `true` when both links point at the same resource revision.
    pub fn same_revision(&self, other: &ResourceLink) -> bool {
        self.kind == other.kind && self.id == other.id && self.revision == other.revision
    }
}

/// One entry in the user-facing timeline.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineEntry {
    /// Append-only position in the timeline. Never reused after a delete.
    pub seq: u64,
    /// What happened.
    pub event_type: TimelineEventType,
    /// Who may read it.
    pub visibility: Visibility,
    /// The resource it happened to.
    pub link: ResourceLink,
    /// Who caused it. `None` for contract-internal events.
    pub actor: Option<Address>,
    /// Whose record it is, when the event has a single beneficiary.
    pub subject: Option<Address>,
    /// Ledger sequence at which it was recorded.
    pub ledger: u32,
    /// Ledger timestamp at which it was recorded.
    pub timestamp: u64,
    /// Short, non-sensitive label the UI may show verbatim. Free-form detail
    /// belongs in the off-chain indexer, never in a user-facing entry.
    pub summary: Symbol,
    /// Set by [`redact_entry`]. Redacted entries keep their `seq` so ordering
    /// stays stable, but are hidden from non-maintainers.
    pub redacted: bool,
}

/// One maintainer-only audit record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEntry {
    /// Append-only position in the audit store (independent of `seq`).
    pub seq: u64,
    /// What happened.
    pub event_type: TimelineEventType,
    /// The resource it happened to.
    pub link: ResourceLink,
    /// The maintainer that recorded or performed it.
    pub actor: Address,
    /// Ledger sequence at which it was recorded.
    pub ledger: u32,
    /// Ledger timestamp at which it was recorded.
    pub timestamp: u64,
    /// Short label.
    pub summary: Symbol,
}

/// Who is asking to read a timeline.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Viewer {
    /// The reader's address, or `None` for an anonymous read.
    pub address: Option<Address>,
    /// Whether that address holds a maintainer role. Only [`viewer_for`]
    /// populates this, and it derives it from on-chain roles — never from
    /// caller-supplied input.
    pub is_maintainer: bool,
}

/// One page of timeline entries, plus the cursor needed to fetch the next one.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelinePage {
    /// Visible entries in `seq` order, oldest first.
    pub entries: Vec<TimelineEntry>,
    /// Pass back as `cursor` to continue. `None` when nothing further remains.
    pub next_cursor: Option<u64>,
    /// `true` when more entries (visible or not) exist beyond this page.
    pub has_more: bool,
}

/// Storage keys owned by this module.
#[contracttype]
pub enum TimelineKey {
    /// Next `seq` to hand out to a user-facing entry.
    NextSeq,
    /// User-facing entry by `seq`.
    Entry(u64),
    /// Next `seq` to hand out to an audit entry.
    NextAuditSeq,
    /// Maintainer-only audit entry by `seq`.
    AuditEntry(u64),
}

// ---------------------------------------------------------------------------
// Viewers
// ---------------------------------------------------------------------------

/// Returns `true` when `who` may read maintainer-only context.
///
/// Maintainer status is `Role::Admin` or `Role::Upgrader`, read from on-chain
/// role storage.
pub fn is_maintainer(env: &Env, who: &Address) -> bool {
    has_role(env, who, Role::Admin) || has_role(env, who, Role::Upgrader)
}

/// Builds the [`Viewer`] for `who` by reading their on-chain roles.
pub fn viewer_for(env: &Env, who: &Address) -> Viewer {
    Viewer {
        address: Some(who.clone()),
        is_maintainer: is_maintainer(env, who),
    }
}

/// Builds a [`Viewer`] with no address — sees exactly the public timeline.
pub fn anonymous_viewer() -> Viewer {
    Viewer {
        address: None,
        is_maintainer: false,
    }
}

/// Decides whether `viewer` may read `entry`.
///
/// This is the single place visibility is enforced, so `timeline_page`,
/// `audit_trail` and any future accessor share one rule set.
pub fn can_view(entry: &TimelineEntry, viewer: &Viewer) -> bool {
    match entry.visibility {
        Visibility::Public => !entry.redacted || viewer.is_maintainer,
        Visibility::Participant => {
            if viewer.is_maintainer {
                return true;
            }
            if entry.redacted {
                return false;
            }
            match &viewer.address {
                Some(who) => {
                    entry.actor.as_ref() == Some(who) || entry.subject.as_ref() == Some(who)
                }
                None => false,
            }
        }
        // Maintainer-only entries live in the audit store; one never appears on
        // the user timeline. Treat it as unreadable here as a fail-safe.
        Visibility::Maintainer => false,
    }
}

// ---------------------------------------------------------------------------
// Sequence counters
// ---------------------------------------------------------------------------

/// Next `seq` that will be assigned to a user-facing entry.
pub fn next_seq(env: &Env) -> u64 {
    persistent_get(env, &TimelineKey::NextSeq).unwrap_or(0)
}

/// Next `seq` that will be assigned to an audit entry.
pub fn next_audit_seq(env: &Env) -> u64 {
    persistent_get(env, &TimelineKey::NextAuditSeq).unwrap_or(0)
}

/// Number of user-facing entries ever appended (including deleted ones).
pub fn entry_count(env: &Env) -> u64 {
    next_seq(env)
}

fn take_seq(env: &Env) -> u64 {
    let seq = next_seq(env);
    persistent_set(env, &TimelineKey::NextSeq, &(seq + 1));
    seq
}

fn take_audit_seq(env: &Env) -> u64 {
    let seq = next_audit_seq(env);
    persistent_set(env, &TimelineKey::NextAuditSeq, &(seq + 1));
    seq
}

fn load_entry(env: &Env, seq: u64) -> Option<TimelineEntry> {
    persistent_get(env, &TimelineKey::Entry(seq))
}

fn load_audit_entry(env: &Env, seq: u64) -> Option<AuditEntry> {
    persistent_get(env, &TimelineKey::AuditEntry(seq))
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

/// Appends an entry to the user-facing timeline.
///
/// `actor` is required to authorise when present; contract-internal events pass
/// `None`.
///
/// Returns [`Error::InvalidArgument`] when `visibility` is
/// [`Visibility::Maintainer`] — that content belongs in [`record_audit_event`].
pub fn append_user_event(
    env: &Env,
    actor: Option<Address>,
    event_type: TimelineEventType,
    visibility: Visibility,
    link: ResourceLink,
    subject: Option<Address>,
    summary: Symbol,
) -> Result<TimelineEntry, Error> {
    if visibility == Visibility::Maintainer {
        return Err(Error::InvalidArgument);
    }
    // Link the anchor into every write so a malformed link (over-long or
    // non-canonical kind) is rejected before it is stored.
    link.anchor(env)?;
    if let Some(who) = &actor {
        who.require_auth();
    }

    let entry = TimelineEntry {
        seq: take_seq(env),
        event_type,
        visibility,
        link,
        actor,
        subject,
        ledger: env.ledger().sequence(),
        timestamp: env.ledger().timestamp(),
        summary,
        redacted: false,
    };
    persistent_set(env, &TimelineKey::Entry(entry.seq), &entry);

    env.events().publish(
        (symbol_short!("timeline"), symbol_short!("appended")),
        (entry.seq, entry.event_type.clone(), entry.link.clone()),
    );
    Ok(entry)
}

/// Appends to the audit store without an authorization check.
///
/// Private: the two callers — [`record_audit_event`] and the redaction /
/// deletion paths — have already authorized the maintainer, so the check is
/// performed once per entry point instead of once per audit write.
fn write_audit_entry(
    env: &Env,
    actor: Address,
    event_type: TimelineEventType,
    link: ResourceLink,
    summary: Symbol,
) -> Result<AuditEntry, Error> {
    link.anchor(env)?;

    let entry = AuditEntry {
        seq: take_audit_seq(env),
        event_type,
        link,
        actor,
        ledger: env.ledger().sequence(),
        timestamp: env.ledger().timestamp(),
        summary,
    };
    persistent_set(env, &TimelineKey::AuditEntry(entry.seq), &entry);

    env.events().publish(
        (symbol_short!("timeline"), symbol_short!("audit")),
        (entry.seq, entry.event_type.clone(), entry.link.clone()),
    );
    Ok(entry)
}

/// Records a maintainer-only audit entry.
///
/// Maintainer-gated: the caller must hold [`Role::Admin`]. Audit entries are
/// written to their own key space and are only readable through
/// [`audit_trail`], so they cannot leak into [`timeline_page`].
pub fn record_audit_event(
    env: &Env,
    maintainer: &Address,
    event_type: TimelineEventType,
    link: ResourceLink,
    summary: Symbol,
) -> Result<AuditEntry, Error> {
    require_role(env, maintainer, Role::Admin)?;
    write_audit_entry(env, maintainer.clone(), event_type, link, summary)
}

/// Hides an entry from every non-maintainer without disturbing ordering.
///
/// The entry keeps its `seq` and stays visible to maintainers, so a redaction
/// is reviewable rather than silent. The action itself is recorded in the audit
/// trail.
pub fn redact_entry(
    env: &Env,
    maintainer: &Address,
    seq: u64,
    reason: Symbol,
) -> Result<(), Error> {
    require_role(env, maintainer, Role::Admin)?;
    let mut entry = load_entry(env, seq).ok_or(Error::NotFound)?;
    entry.redacted = true;
    let link = entry.link.clone();
    persistent_set(env, &TimelineKey::Entry(seq), &entry);

    write_audit_entry(
        env,
        maintainer.clone(),
        TimelineEventType::RecordRestricted,
        link,
        reason.clone(),
    )?;
    env.events().publish(
        (symbol_short!("timeline"), symbol_short!("redacted")),
        (seq, reason),
    );
    Ok(())
}

/// Removes an entry from the user timeline entirely.
///
/// The `seq` is retired, never reused, so pagination cursors stay valid: a
/// client holding a cursor simply stops seeing that entry rather than being
/// shifted onto a different one. A maintainer record of the deletion is written
/// to the audit trail, so removal is auditable even though the user entry is
/// gone.
pub fn delete_entry(
    env: &Env,
    maintainer: &Address,
    seq: u64,
    reason: Symbol,
) -> Result<(), Error> {
    require_role(env, maintainer, Role::Admin)?;
    let entry = load_entry(env, seq).ok_or(Error::NotFound)?;
    let link = entry.link.clone();
    persistent_remove(env, &TimelineKey::Entry(seq));

    write_audit_entry(
        env,
        maintainer.clone(),
        TimelineEventType::RecordUpdated,
        link,
        reason.clone(),
    )?;

    env.events().publish(
        (symbol_short!("timeline"), symbol_short!("deleted")),
        (seq, reason),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

/// Returns a page of the user-facing timeline visible to `viewer`.
///
/// - `cursor` is the `seq` returned by a previous call's `next_cursor`.
/// - `limit` is the maximum number of entries returned; `0` selects
///   [`DEFAULT_PAGE_SIZE`], and anything above [`MAX_PAGE_SIZE`] is rejected
///   with [`Error::InvalidArgument`] so one read cannot be made unbounded.
pub fn timeline_page(
    env: &Env,
    viewer: &Viewer,
    cursor: Option<u64>,
    limit: u32,
) -> Result<TimelinePage, Error> {
    let limit = if limit == 0 { DEFAULT_PAGE_SIZE } else { limit };
    if limit > MAX_PAGE_SIZE {
        return Err(Error::InvalidArgument);
    }

    // `seq` is zero-based, so `cursor` is the last sequence number the caller
    // already saw and the next request resumes at `cursor + 1`.
    let total = next_seq(env);
    let mut next = match cursor {
        Some(seen) => seen + 1,
        None => 0,
    };
    let mut entries = Vec::new(env);
    let mut scanned: u64 = 0;

    while next < total && scanned < MAX_SCAN_PER_PAGE && entries.len() < limit {
        if let Some(entry) = load_entry(env, next) {
            if can_view(&entry, viewer) {
                entries.push_back(entry);
            }
        }
        next += 1;
        scanned += 1;
    }

    let has_more = next < total;
    // Resume from the last sequence number actually examined, which may be a
    // hidden or deleted one — that is what keeps a filtered page from skipping
    // entries when the caller comes back.
    let next_cursor = if has_more || !entries.is_empty() {
        Some(next.saturating_sub(1))
    } else {
        None
    };

    Ok(TimelinePage {
        entries,
        next_cursor,
        has_more,
    })
}

/// Returns maintainer-only audit entries, newest first.
///
/// Maintainer-gated: the caller must hold [`Role::Admin`]. This is the only
/// accessor for the audit store, and it never returns user-facing entries.
pub fn audit_trail(env: &Env, maintainer: &Address, limit: u32) -> Result<Vec<AuditEntry>, Error> {
    require_role(env, maintainer, Role::Admin)?;
    let limit = if limit == 0 { DEFAULT_PAGE_SIZE } else { limit };
    if limit > MAX_PAGE_SIZE {
        return Err(Error::InvalidArgument);
    }

    let total = next_audit_seq(env);
    let mut out = Vec::new(env);
    let mut seq = total;
    while seq > 0 && out.len() < limit {
        seq -= 1;
        if let Some(entry) = load_audit_entry(env, seq) {
            out.push_back(entry);
        }
    }
    Ok(out)
}

/// `true` when an entry with this `seq` exists and has not been deleted.
pub fn entry_exists(env: &Env, seq: u64) -> bool {
    persistent_has(env, &TimelineKey::Entry(seq))
}

/// `true` when the entry exists and has been redacted.
pub fn entry_is_redacted(env: &Env, seq: u64) -> bool {
    load_entry(env, seq).map(|e| e.redacted).unwrap_or(false)
}
