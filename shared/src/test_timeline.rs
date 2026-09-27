#![cfg(test)]

extern crate std;

use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Ledger as _},
    Address, Bytes, Env, Vec,
};

use crate::{
    auth::{grant_role, set_admin, Role},
    errors::Error,
    timeline::{
        anonymous_viewer, append_user_event, audit_trail, can_view, delete_entry, entry_count,
        entry_exists, entry_is_redacted, is_maintainer, record_audit_event, redact_entry,
        timeline_page, viewer_for, ResourceLink, TimelineEntry, TimelineEventType, Visibility,
        DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE,
    },
};

#[contract]
pub struct DummyTimelineContract;

#[contractimpl]
impl DummyTimelineContract {
    pub fn noop(_env: Env) {}
}

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

struct Ctx {
    env: Env,
    id: Address,
    admin: Address,
    alice: Address,
    bob: Address,
    stranger: Address,
}

impl Ctx {
    /// Runs `f` in its own contract frame.
    ///
    /// Production callers reach this module through contract invocations, which
    /// each get a frame; the test host refuses a second `require_auth` for the
    /// same address inside one frame, so every operation gets its own.
    fn run<T>(&self, f: impl FnOnce() -> T) -> T {
        self.env.as_contract(&self.id, f)
    }
}

/// Fresh env, registered dummy contract, one admin holding `Role::Admin` and
/// three unrelated addresses.
fn setup() -> Ctx {
    let env = Env::default();
    // Contract-internal helpers call `require_auth` directly rather than via a
    // root invocation, so authorizations are mocked for any address.
    env.mock_all_auths_allowing_non_root_auth();
    let admin = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let stranger = Address::generate(&env);
    let id = env.register_contract(None, DummyTimelineContract);
    env.as_contract(&id, || {
        set_admin(&env, &admin);
        grant_role(&env, &admin, &admin, Role::Admin).unwrap();
    });
    Ctx {
        env,
        id,
        admin,
        alice,
        bob,
        stranger,
    }
}

fn link(env: &Env, id: u64, revision: u32) -> ResourceLink {
    ResourceLink {
        kind: Bytes::from_slice(env, b"aid"),
        id,
        revision,
    }
}

fn append_public(ctx: &Ctx, id: u64) -> TimelineEntry {
    ctx.run(|| public(&ctx.env, id, &ctx.alice))
}

fn public(env: &Env, id: u64, actor: &Address) -> TimelineEntry {
    append_user_event(
        env,
        Some(actor.clone()),
        TimelineEventType::RecordCreated,
        Visibility::Public,
        link(env, id, 0),
        None,
        symbol_short!("created"),
    )
    .unwrap()
}

fn append_participant(ctx: &Ctx, id: u64) -> TimelineEntry {
    ctx.run(|| {
        append_user_event(
            &ctx.env,
            Some(ctx.alice.clone()),
            TimelineEventType::PaymentSent,
            Visibility::Participant,
            link(&ctx.env, id, 0),
            Some(ctx.bob.clone()),
            symbol_short!("paid"),
        )
        .unwrap()
    })
}

/// Sequence numbers of a page, as a `std` vec for easy assertions.
fn seqs(entries: &Vec<TimelineEntry>) -> std::vec::Vec<u64> {
    let mut out = std::vec::Vec::new();
    for e in entries.iter() {
        out.push(e.seq);
    }
    out
}

/// Reads a page for `viewer`.
fn page_for(
    ctx: &Ctx,
    viewer: &crate::timeline::Viewer,
    cursor: Option<u64>,
    limit: u32,
) -> crate::timeline::TimelinePage {
    ctx.run(|| timeline_page(&ctx.env, viewer, cursor, limit).unwrap())
}

// ---------------------------------------------------------------------------
// 1. Append semantics
// ---------------------------------------------------------------------------

#[test]
fn append_assigns_monotonic_seq_and_stamps_ledger() {
    let ctx = setup();
    ctx.run(|| {
        ctx.env.ledger().set_timestamp(1_700);
        ctx.env.ledger().set_sequence_number(64);
    });

    let first = append_public(&ctx, 1);
    let second = append_public(&ctx, 2);

    assert_eq!(first.seq, 0, "first entry starts the sequence at zero");
    assert_eq!(second.seq, 1, "sequence is gap-free and monotonic");
    assert_eq!(first.ledger, 64);
    assert_eq!(first.timestamp, 1_700);
    assert!(!first.redacted);
    assert_eq!(ctx.run(|| entry_count(&ctx.env)), 2);
}

#[test]
fn append_rejects_maintainer_visibility() {
    let ctx = setup();
    let res = ctx.run(|| {
        append_user_event(
            &ctx.env,
            Some(ctx.admin.clone()),
            TimelineEventType::ConfigChanged,
            Visibility::Maintainer,
            link(&ctx.env, 1, 0),
            None,
            symbol_short!("cfg"),
        )
    });
    assert_eq!(
        res,
        Err(Error::InvalidArgument),
        "maintainer-only content must go through record_audit_event"
    );
    assert_eq!(ctx.run(|| entry_count(&ctx.env)), 0, "nothing was stored");
}

#[test]
fn append_requires_actor_authorisation() {
    // A fresh env without any auth mocking: `require_auth` must fail.
    let env = Env::default();
    let alice = Address::generate(&env);
    let id = env.register_contract(None, DummyTimelineContract);
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        env.as_contract(&id, || {
            append_user_event(
                &env,
                Some(alice.clone()),
                TimelineEventType::RecordCreated,
                Visibility::Public,
                link(&env, 1, 0),
                None,
                symbol_short!("created"),
            )
        })
    }));
    assert!(
        res.is_err(),
        "an unauthorised actor must not be able to append"
    );
}

// ---------------------------------------------------------------------------
// 2. Visibility boundaries
// ---------------------------------------------------------------------------

#[test]
fn anonymous_viewer_sees_only_public_entries() {
    let ctx = setup();
    append_public(&ctx, 1);
    append_participant(&ctx, 2);

    let page = page_for(&ctx, &anonymous_viewer(), None, 0);
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries.get(0).unwrap().seq, 0);
}

#[test]
fn participant_entries_are_visible_to_actor_and_subject_only() {
    let ctx = setup();
    append_participant(&ctx, 7);

    assert_eq!(
        page_for(&ctx, &anonymous_viewer(), None, 0).entries.len(),
        0,
        "anonymous callers see nothing"
    );

    for (who, expected) in [(&ctx.alice, true), (&ctx.bob, true), (&ctx.stranger, false)] {
        let viewer = ctx.run(|| viewer_for(&ctx.env, who));
        let page = page_for(&ctx, &viewer, None, 0);
        assert_eq!(
            page.entries.len() == 1,
            expected,
            "unexpected visibility for viewer"
        );
    }
}

#[test]
fn maintainer_viewer_sees_every_user_entry() {
    let ctx = setup();
    assert!(ctx.run(|| is_maintainer(&ctx.env, &ctx.admin)));
    assert!(!ctx.run(|| is_maintainer(&ctx.env, &ctx.stranger)));

    append_public(&ctx, 1);
    append_participant(&ctx, 2);

    let viewer = ctx.run(|| viewer_for(&ctx.env, &ctx.admin));
    let page = page_for(&ctx, &viewer, None, 0);
    assert_eq!(page.entries.len(), 2);
    assert!(!page.has_more);
    assert_eq!(page.next_cursor, Some(1));
}

#[test]
fn visibility_rule_denies_maintainer_entries_on_the_user_timeline() {
    let ctx = setup();
    // Fail-safe: even if such an entry somehow existed in the timeline store,
    // no viewer — not even a maintainer — reads it there.
    ctx.run(|| {
        let forged = TimelineEntry {
            seq: 0,
            event_type: TimelineEventType::ConfigChanged,
            visibility: Visibility::Maintainer,
            link: link(&ctx.env, 1, 0),
            actor: None,
            subject: None,
            ledger: 0,
            timestamp: 0,
            summary: symbol_short!("cfg"),
            redacted: false,
        };
        assert!(!can_view(&forged, &anonymous_viewer()));
        assert!(!can_view(&forged, &viewer_for(&ctx.env, &ctx.admin)));
    });
}

// ---------------------------------------------------------------------------
// 3. Maintainer-only audit store is separate
// ---------------------------------------------------------------------------

#[test]
fn audit_entries_never_appear_in_the_user_timeline() {
    let ctx = setup();
    append_public(&ctx, 1);
    ctx.run(|| {
        record_audit_event(
            &ctx.env,
            &ctx.admin,
            TimelineEventType::ConfigChanged,
            link(&ctx.env, 1, 0),
            symbol_short!("cfg"),
        )
        .unwrap()
    });

    let viewer = ctx.run(|| viewer_for(&ctx.env, &ctx.admin));
    let page = page_for(&ctx, &viewer, None, 0);
    assert_eq!(
        page.entries.len(),
        1,
        "only the user entry is on the timeline"
    );

    let audit = ctx.run(|| audit_trail(&ctx.env, &ctx.admin, 0).unwrap());
    assert_eq!(audit.len(), 1);
    assert_eq!(audit.get(0).unwrap().actor, ctx.admin);
}

#[test]
fn audit_trail_requires_admin_role() {
    let ctx = setup();
    assert_eq!(
        ctx.run(|| audit_trail(&ctx.env, &ctx.stranger, 0)),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        ctx.run(|| record_audit_event(
            &ctx.env,
            &ctx.stranger,
            TimelineEventType::ConfigChanged,
            link(&ctx.env, 1, 0),
            symbol_short!("cfg"),
        )),
        Err(Error::Unauthorized)
    );
}

// ---------------------------------------------------------------------------
// 4. Pagination
// ---------------------------------------------------------------------------

#[test]
fn pagination_is_stable_and_ordered() {
    let ctx = setup();
    for id in 0..7u64 {
        append_public(&ctx, id);
    }

    let page1 = page_for(&ctx, &anonymous_viewer(), None, 3);
    assert_eq!(page1.entries.len(), 3);
    assert_eq!(
        seqs(&page1.entries),
        std::vec![0u64, 1, 2],
        "oldest first, in sequence order"
    );
    assert!(page1.has_more);
    assert_eq!(page1.next_cursor, Some(2));

    let page2 = page_for(&ctx, &anonymous_viewer(), page1.next_cursor, 3);
    assert_eq!(seqs(&page2.entries), std::vec![3u64, 4, 5]);
    assert!(page2.has_more);
    assert_eq!(page2.next_cursor, Some(5));

    let page3 = page_for(&ctx, &anonymous_viewer(), page2.next_cursor, 3);
    assert_eq!(seqs(&page3.entries), std::vec![6u64]);
    assert!(!page3.has_more);
    assert_eq!(page3.next_cursor, Some(6));

    // Resuming past the end terminates instead of looping.
    let page4 = page_for(&ctx, &anonymous_viewer(), page3.next_cursor, 3);
    assert_eq!(page4.entries.len(), 0);
    assert!(!page4.has_more);
    assert_eq!(page4.next_cursor, None);
}

#[test]
fn zero_limit_selects_the_default_page_size() {
    let ctx = setup();
    for id in 0..(DEFAULT_PAGE_SIZE + 5) {
        append_public(&ctx, id as u64);
    }
    let page = page_for(&ctx, &anonymous_viewer(), None, 0);
    assert_eq!(page.entries.len(), DEFAULT_PAGE_SIZE);
    assert!(page.has_more);
}

#[test]
fn oversized_page_is_rejected() {
    let ctx = setup();
    assert_eq!(
        ctx.run(|| timeline_page(&ctx.env, &anonymous_viewer(), None, MAX_PAGE_SIZE + 1)),
        Err(Error::InvalidArgument)
    );
    assert_eq!(
        ctx.run(|| audit_trail(&ctx.env, &ctx.admin, MAX_PAGE_SIZE + 1)),
        Err(Error::InvalidArgument)
    );
}

// ---------------------------------------------------------------------------
// 5. Deleted and restricted records
// ---------------------------------------------------------------------------

#[test]
fn deleted_entries_vanish_without_shifting_the_sequence() {
    let ctx = setup();
    for id in 0..5u64 {
        append_public(&ctx, id);
    }
    ctx.run(|| delete_entry(&ctx.env, &ctx.admin, 2, symbol_short!("gdpr")).unwrap());

    assert!(!ctx.run(|| entry_exists(&ctx.env, 2)), "the entry is gone");
    assert_eq!(
        ctx.run(|| entry_count(&ctx.env)),
        5,
        "the sequence is not renumbered"
    );

    let mut seen = std::vec::Vec::new();
    let mut cursor = None;
    loop {
        let page = page_for(&ctx, &anonymous_viewer(), cursor, 2);
        seen.extend(seqs(&page.entries));
        match page.next_cursor {
            Some(next) if page.has_more => cursor = Some(next),
            _ => break,
        }
    }
    assert_eq!(
        seen,
        std::vec![0u64, 1, 3, 4],
        "deleted seq is skipped, everything else keeps its position"
    );

    assert_eq!(
        ctx.run(|| delete_entry(&ctx.env, &ctx.admin, 99, symbol_short!("gdpr"))),
        Err(Error::NotFound)
    );
}

#[test]
fn redaction_hides_entries_from_participants_but_not_maintainers() {
    let ctx = setup();
    append_participant(&ctx, 3);

    let alice = ctx.run(|| viewer_for(&ctx.env, &ctx.alice));
    assert_eq!(page_for(&ctx, &alice, None, 0).entries.len(), 1);

    ctx.run(|| redact_entry(&ctx.env, &ctx.admin, 0, symbol_short!("hold")).unwrap());
    assert!(ctx.run(|| entry_is_redacted(&ctx.env, 0)));

    assert_eq!(
        page_for(&ctx, &alice, None, 0).entries.len(),
        0,
        "the participant loses visibility"
    );
    assert_eq!(
        page_for(&ctx, &anonymous_viewer(), None, 0).entries.len(),
        0
    );
    let admin = ctx.run(|| viewer_for(&ctx.env, &ctx.admin));
    assert_eq!(
        page_for(&ctx, &admin, None, 0).entries.len(),
        1,
        "maintainers keep the audit view of a redacted entry"
    );
    assert_eq!(
        ctx.run(|| redact_entry(&ctx.env, &ctx.stranger, 0, symbol_short!("hold"))),
        Err(Error::Unauthorized)
    );
}

#[test]
fn redaction_and_deletion_are_recorded_in_the_audit_trail() {
    let ctx = setup();
    append_public(&ctx, 1);
    append_public(&ctx, 2);

    ctx.run(|| redact_entry(&ctx.env, &ctx.admin, 0, symbol_short!("hold")).unwrap());
    ctx.run(|| delete_entry(&ctx.env, &ctx.admin, 1, symbol_short!("gdpr")).unwrap());

    let audit = ctx.run(|| audit_trail(&ctx.env, &ctx.admin, 0).unwrap());
    assert_eq!(audit.len(), 2, "each action leaves a maintainer record");
    assert_eq!(
        audit.get(0).unwrap().event_type,
        TimelineEventType::RecordUpdated,
        "newest first: the deletion"
    );
    assert_eq!(
        audit.get(1).unwrap().event_type,
        TimelineEventType::RecordRestricted
    );
    for entry in audit.iter() {
        assert_eq!(entry.actor, ctx.admin);
    }
}

// ---------------------------------------------------------------------------
// 6. Stable resource links
// ---------------------------------------------------------------------------

#[test]
fn resource_anchors_are_deterministic_and_revision_sensitive() {
    let ctx = setup();
    ctx.run(|| {
        let a = link(&ctx.env, 42, 1).anchor(&ctx.env).unwrap();
        let b = link(&ctx.env, 42, 1).anchor(&ctx.env).unwrap();
        let bumped = link(&ctx.env, 42, 2).anchor(&ctx.env).unwrap();
        let other = link(&ctx.env, 43, 1).anchor(&ctx.env).unwrap();

        assert_eq!(a, b, "the same resource and revision anchor identically");
        assert_ne!(a, bumped, "a revision bump yields a new anchor");
        assert_ne!(a, other, "a different resource yields a new anchor");
        assert!(link(&ctx.env, 42, 1).same_revision(&link(&ctx.env, 42, 1)));
        assert!(!link(&ctx.env, 42, 1).same_revision(&link(&ctx.env, 42, 2)));
    });
}

#[test]
fn append_rejects_a_non_canonical_resource_kind() {
    let ctx = setup();
    let res = ctx.run(|| {
        append_user_event(
            &ctx.env,
            Some(ctx.alice.clone()),
            TimelineEventType::RecordCreated,
            Visibility::Public,
            ResourceLink {
                kind: Bytes::from_slice(&ctx.env, &[b'x'; 200]),
                id: 1,
                revision: 0,
            },
            None,
            symbol_short!("created"),
        )
    });
    assert!(
        res.is_err(),
        "an unbounded resource kind must not be stored"
    );
}
