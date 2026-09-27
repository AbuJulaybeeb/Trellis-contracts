# Lifecycle State Machine Design

## Issue Context
**Issue #27**: Implement deterministic lifecycle state machine for core records

## Problem Statement
Currently, the codebase relies on scattered boolean flags, implicit status checks, and inconsistent state transitions across three core record types:
- `AidRecord`
- `EscrowRecord`
- `Proposal`

This creates maintenance challenges and potential bugs due to:
- Scattered validation logic
- No centralized transition guards
- Inconsistent error handling
- Difficult-to-audit state changes

## Solution: Unified Lifecycle State Machine

### Design Principles

1. **Explicit State Transitions**: All valid transitions are defined in a central location
2. **Guarded Transitions**: Invalid transitions are rejected at compile-time or runtime
3. **Audit Trail**: Every state change emits an event for indexing and compliance
4. **Type Safety**: Leverage Rust's type system to prevent invalid states
5. **Backward Compatibility**: Existing storage remains compatible

### Core Records & Their Lifecycles

#### 1. AidRecord Lifecycle

**States:**
```rust
pub enum AidStatus {
    Pending,    // Initial state: funds escrowed, awaiting claim
    Settled,    // Terminal state: claimed by recipient
    Refunded,   // Terminal state: returned to donor after expiry
}
```

**Valid Transitions:**
```
Pending → Settled    (when: recipient claims before expiry)
Pending → Refunded   (when: donor refunds after expiry)
```

**Invalid Transitions (must be rejected):**
```
Settled  → Pending   ❌
Settled  → Refunded  ❌
Refunded → Pending   ❌
Refunded → Settled   ❌
Settled  → Settled   ❌ (idempotency check)
Refunded → Refunded  ❌ (idempotency check)
```

**Business Rules:**
- `Pending → Settled`: Requires current_ledger <= expiry_ledger AND caller == recipient
- `Pending → Refunded`: Requires current_ledger > expiry_ledger

#### 2. EscrowRecord Lifecycle

**States:**
```rust
pub enum EscrowState {
    Active,     // Initial state: funds held in contract
    Released,   // Terminal state: transferred to beneficiary
    Refunded,   // Terminal state: returned to depositor
}
```

**Valid Transitions:**
```
Active → Released    (when: authorized party releases before/at expiry)
Active → Refunded    (when: authorized party refunds, typically after expiry)
```

**Invalid Transitions (must be rejected):**
```
Released → Active    ❌
Released → Refunded  ❌
Refunded → Active    ❌
Refunded → Released  ❌
Released → Released  ❌
Refunded → Refunded  ❌
```

**Business Rules:**
- `Active → Released`: Requires current_ledger <= expiry_ledger
- `Active → Refunded`: Typically after expiry, but admin may override

#### 3. Proposal Lifecycle

**States:**
```rust
pub enum ProposalStatus {
    Pending,    // Initial state: awaiting approvals
    Executed,   // Terminal state: action has been performed
}
```

**Valid Transitions:**
```
Pending → Executed   (when: approval_count >= threshold)
```

**Invalid Transitions (must be rejected):**
```
Executed → Pending   ❌
Executed → Executed  ❌
```

**Business Rules:**
- `Pending → Executed`: Requires approval_count >= threshold

### Implementation Architecture

#### Shared State Machine Module

Create `shared/src/lifecycle.rs` with:

```rust
pub trait StateMachine {
    type State: Copy + Clone + Eq;
    type TransitionContext;
    type Error;

    /// Check if a transition from current to next state is valid
    fn can_transition(
        current: Self::State,
        next: Self::State,
        context: &Self::TransitionContext,
    ) -> Result<(), Self::Error>;

    /// Execute a state transition with validation and event emission
    fn transition(
        current: Self::State,
        next: Self::State,
        context: &Self::TransitionContext,
    ) -> Result<Self::State, Self::Error>;
}
```

#### Concrete Implementations

**AidStateMachine:**
```rust
pub struct AidStateMachine;

pub struct AidTransitionContext {
    pub current_ledger: u32,
    pub expiry_ledger: u32,
    pub caller: Address,
    pub recipient: Address,
}

impl StateMachine for AidStateMachine {
    type State = AidStatus;
    type TransitionContext = AidTransitionContext;
    type Error = AidError;

    fn can_transition(
        current: AidStatus,
        next: AidStatus,
        ctx: &AidTransitionContext,
    ) -> Result<(), AidError> {
        match (current, next) {
            (AidStatus::Pending, AidStatus::Settled) => {
                if ctx.current_ledger > ctx.expiry_ledger {
                    return Err(AidError::Expired);
                }
                if ctx.caller != ctx.recipient {
                    return Err(AidError::Unauthorized);
                }
                Ok(())
            }
            (AidStatus::Pending, AidStatus::Refunded) => {
                if ctx.current_ledger <= ctx.expiry_ledger {
                    return Err(AidError::NotExpiredYet);
                }
                Ok(())
            }
            (AidStatus::Settled, _) => Err(AidError::AlreadyClaimed),
            (AidStatus::Refunded, _) => Err(AidError::AlreadyRefunded),
            _ => Err(AidError::InvalidTransition),
        }
    }
}
```

**EscrowStateMachine:**
```rust
pub struct EscrowStateMachine;

pub struct EscrowTransitionContext {
    pub current_ledger: u32,
    pub expiry_ledger: u32,
}

impl StateMachine for EscrowStateMachine {
    type State = EscrowState;
    type TransitionContext = EscrowTransitionContext;
    type Error = Error;

    fn can_transition(
        current: EscrowState,
        next: EscrowState,
        ctx: &EscrowTransitionContext,
    ) -> Result<(), Error> {
        match (current, next) {
            (EscrowState::Active, EscrowState::Released) => {
                if ctx.current_ledger > ctx.expiry_ledger {
                    return Err(Error::PaymentEscrowExpired);
                }
                Ok(())
            }
            (EscrowState::Active, EscrowState::Refunded) => Ok(()),
            (EscrowState::Released, _) => Err(Error::PaymentEscrowAlreadyReleased),
            (EscrowState::Refunded, _) => Err(Error::PaymentEscrowAlreadyRefunded),
            _ => Err(Error::InvalidTransition),
        }
    }
}
```

**ProposalStateMachine:**
```rust
pub struct ProposalStateMachine;

pub struct ProposalTransitionContext {
    pub approval_count: u32,
    pub threshold: u32,
}

impl StateMachine for ProposalStateMachine {
    type State = ProposalStatus;
    type TransitionContext = ProposalTransitionContext;
    type Error = Error;

    fn can_transition(
        current: ProposalStatus,
        next: ProposalStatus,
        ctx: &ProposalTransitionContext,
    ) -> Result<(), Error> {
        match (current, next) {
            (ProposalStatus::Pending, ProposalStatus::Executed) => {
                if ctx.approval_count < ctx.threshold {
                    return Err(Error::BelowThreshold);
                }
                Ok(())
            }
            (ProposalStatus::Executed, _) => Err(Error::AlreadyExecuted),
            _ => Err(Error::InvalidTransition),
        }
    }
}
```

### Event Emission Strategy

Every state transition emits a structured event:

```rust
pub fn emit_state_transition<S: Into<Symbol>>(
    env: &Env,
    record_type: Symbol,      // "aid", "escrow", "proposal"
    record_id: u64,
    from_state: S,
    to_state: S,
    actor: &Address,
    timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("state"), symbol_short!("trans")),
        StateTransitionEvent {
            record_type,
            record_id,
            from_state: from_state.into(),
            to_state: to_state.into(),
            actor: actor.clone(),
            timestamp,
        },
    );
}
```

### Migration Strategy

#### Phase 1: Add State Machine (Non-Breaking)
- Create `shared/src/lifecycle.rs` module
- Implement state machine traits and validators
- Keep existing direct state assignments

#### Phase 2: Refactor Contracts (Breaking for Logic, Not Storage)
- Replace scattered checks with `can_transition()` calls
- Add event emissions for all transitions
- Storage format remains unchanged

#### Phase 3: Testing & Validation
- Unit tests for all valid transitions
- Unit tests for at least 5 invalid transitions per record type
- Integration tests for full lifecycles
- Property-based tests for state invariants

### Error Handling

Add new error variant to `shared/src/errors.rs`:

```rust
pub enum Error {
    // ... existing errors ...
    InvalidTransition = 950,  // Attempted an illegal state transition
}
```

### Test Coverage Requirements

**Per Issue #27 Acceptance Criteria:**
- ✅ Invalid state transitions are consistently rejected
- ✅ Test coverage includes all valid transitions
- ✅ At least 5 rejected scenarios per record type

**Test Matrix:**

| Record Type | Valid Transitions | Invalid Transitions to Test |
|-------------|-------------------|----------------------------|
| Aid         | 2                 | 6+ (all terminal loops + cross-transitions) |
| Escrow      | 2                 | 6+ (all terminal loops + cross-transitions) |
| Proposal    | 1                 | 2+ (terminal loop + reverse) |

### Documentation Updates

1. **API Docs**: Document state machine behavior in each contract's rustdoc
2. **Migration Guide**: Add `docs/LIFECYCLE_MIGRATION.md` for upgrading contracts
3. **Contributor Guide**: Update `CONTRIBUTING.md` with state machine patterns

### Tradeoffs & Alternatives Considered

#### Alternative 1: Enum-Based State Machine with Match Exhaustiveness
**Pros**: Compile-time exhaustiveness checking
**Cons**: Less flexible for runtime context validation
**Decision**: Use trait-based approach for runtime business rule validation

#### Alternative 2: Separate State Machine Contract
**Pros**: Single source of truth, reusable across contracts
**Cons**: Extra cross-contract call overhead, more complex
**Decision**: Use shared library module for zero-overhead validation

#### Alternative 3: Add Timestamps to State Transitions
**Pros**: Better auditability
**Cons**: Storage overhead
**Decision**: Emit events with timestamps instead of storing in records

### Deployment Plan

1. **Development**: Implement in feature branch `fix/issue-27`
2. **Testing**: Run full test suite including new state machine tests
3. **Review**: Architectural review focusing on:
   - State transition correctness
   - Event emission completeness
   - Backward compatibility
4. **Deployment**: Upgrade contracts using existing upgrade mechanism
5. **Monitoring**: Track state transition events in indexer

### Success Metrics

- ✅ Zero invalid transitions in production (via monitoring)
- ✅ 100% test coverage for valid transitions
- ✅ At least 5 invalid transition tests per record type
- ✅ All scattered boolean checks replaced with state machine calls
- ✅ Event emission for every state change

### Open Questions

1. Should we add a `Cancelled` state for Aid/Escrow before expiry?
   - **Decision**: Not in initial implementation; add if needed later

2. Should Proposal support a `Rejected` terminal state?
   - **Decision**: Not in initial implementation; current design is sufficient

3. Should we version the state machine for future extensions?
   - **Decision**: Use the existing schema versioning system in `shared/src/compat.rs`

---

**Author**: AI Assistant
**Date**: 2026-09-27
**Status**: Design Complete, Ready for Implementation
