# Activity Timeline

`shared::timeline` renders a **user-facing activity timeline** for Trellis
records, filtered by who is asking.

Ledger events are written for integrators: they are cheap, unordered from a
user's perspective, and carry as much detail as possible. A user asking "what
happened to my aid?" needs the opposite: an ordered, paginated, *filtered* view
that cannot leak operational context. That is what this module provides.

## Two stores, deliberately separate

| Store             | Written by             | Read by              | Storage key           |
|-------------------|------------------------|----------------------|-----------------------|
| User timeline     | `append_user_event`    | `timeline_page`      | `TimelineKey::Entry`  |
| Maintainer audit  | `record_audit_event`   | `audit_trail`        | `TimelineKey::AuditEntry` |

The separation is structural, not a convention:

- the two stores live under different keys and have different accessors;
- there is no code path from `TimelineKey::AuditEntry` into `timeline_page`;
- `append_user_event` **rejects** `Visibility::Maintainer` with
  `Error::InvalidArgument`, so the flag cannot be flipped to smuggle
  maintainer-only content onto the user timeline.

## Visibility rules

`can_view` is the single place the rules are enforced, so every accessor shares
one implementation.

| `Visibility`   | Anonymous | Participant | Maintainer |
|----------------|-----------|-------------|------------|
| `Public`       | ✅        | ✅          | ✅         |
| `Participant`  | ❌        | ✅          | ✅         |
| `Maintainer`   | ❌        | ❌          | audit only |

- **Participant** — the viewer is the entry's `actor` or its `subject`.
- **Maintainer** — the viewer holds `Role::Admin` or `Role::Upgrader`, read from
  on-chain role storage by `is_maintainer`. The `Viewer` struct is only ever
  populated by `viewer_for`, which derives this from storage — never from
  caller-supplied input.
- A **redacted** entry is visible only to maintainers, whatever its visibility
  was originally.

## Stable links

Every entry carries a `ResourceLink { kind, id, revision }`. `anchor()` returns a
deterministic `BytesN<32>` built with the workspace's canonical encoding
(`shared::canonical`), so:

- the same resource revision always produces the same anchor;
- bumping `revision` always produces a different anchor, so a link captured in a
  UI can never silently resolve to content that has since changed.

`append_user_event` computes the anchor on write, so a non-canonical or
over-long `kind` is rejected before anything is stored.

## Ordering and pagination

Each entry takes the next `seq` from an append-only counter. Pages are returned
in `seq` order and are addressed by the `seq` of the last entry the caller saw.

Consequences, all covered by tests:

- redacting or deleting an entry **never renumbers or reorders** anything else;
- resuming from a cursor never repeats or skips an entry;
- concurrent appends only extend the sequence.

```rust
let mut cursor = None;
loop {
    let page = timeline_page(&env, &viewer, cursor, 20)?;
    for entry in page.entries.iter() { render(entry); }
    if !page.has_more { break; }
    cursor = page.next_cursor;
}
```

### Bounded reads

| Constant             | Value | Meaning                                     |
|----------------------|-------|---------------------------------------------|
| `DEFAULT_PAGE_SIZE`  | 20    | used when a caller passes `0`               |
| `MAX_PAGE_SIZE`      | 50    | larger requests are rejected                |
| `MAX_SCAN_PER_PAGE`  | 256   | sequence numbers examined per request       |

When a filter hides most entries, a request returns a **short page** plus a
`next_cursor` instead of scanning unbounded storage. Callers must treat
`has_more`, not "a full page", as the signal to continue.

## API

```rust
// Writes
append_user_event(env, actor, event_type, visibility, link, subject, summary) -> Result<TimelineEntry, Error>
record_audit_event(env, maintainer, event_type, link, summary)      -> Result<AuditEntry, Error>
redact_entry(env, maintainer, seq, reason)                         -> Result<(), Error>
delete_entry(env, maintainer, seq, reason)                         -> Result<(), Error>

// Reads
timeline_page(env, viewer, cursor, limit) -> Result<TimelinePage, Error>
audit_trail(env, maintainer, limit)       -> Result<Vec<AuditEntry>, Error>

// Viewers
viewer_for(env, address) -> Viewer
anonymous_viewer()       -> Viewer
is_maintainer(env, address) -> bool
can_view(entry, viewer)     -> bool
```

### Authorization

| Operation                                    | Requirement                       |
|----------------------------------------------|-----------------------------------|
| `append_user_event` with an `actor`           | that actor's `require_auth()`     |
| `append_user_event` with `actor: None`        | contract-internal only            |
| `record_audit_event`, `redact_entry`, `delete_entry` | `Role::Admin`              |
| `audit_trail`                                 | `Role::Admin`                     |
| `timeline_page`                               | none — filtered per viewer        |

### Errors

Only existing `shared::Error` codes are used, so the workspace error-code ranges
stay stable for integrators:

| Code                | Raised when                                            |
|---------------------|--------------------------------------------------------|
| `Unauthorized`      | a maintainer-gated call comes from a non-maintainer     |
| `InvalidArgument`   | `Visibility::Maintainer` on the user timeline; `limit > MAX_PAGE_SIZE`; non-canonical `ResourceLink.kind` |
| `NotFound`          | `redact_entry` / `delete_entry` for an unknown `seq`    |

## Deleting and redacting records

Both are maintainer actions and both write an audit entry, so a removal is
reviewable rather than silent.

- **`redact_entry(seq, reason)`** — the entry keeps its `seq` and stays visible
  to maintainers, and is hidden from everyone else. Ordering is untouched.
- **`delete_entry(seq, reason)`** — the entry is removed from the user timeline
  and its `seq` is retired. The sequence is *not* renumbered and the `seq` is
  never reused, which is what keeps outstanding cursors meaningful.

## Testing

The module ships with `shared/src/test_timeline.rs`, covering:

- append semantics (`seq` monotonicity, ledger stamps, `require_auth`);
- visibility boundaries (anonymous / participant / maintainer);
- the audit store never appearing on the user timeline;
- pagination stability, cursor termination, and gaps from deleted entries;
- deleted and redacted records, including audit-trail evidence;
- anchor determinism and revision sensitivity.

```bash
cargo test -p shared timeline
```
