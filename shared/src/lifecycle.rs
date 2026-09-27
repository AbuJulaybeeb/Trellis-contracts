//! # Lifecycle State Machine
//!
//! Provides deterministic state transition validation for core records in the
//! Trellis contract suite. Implements Issue #27's requirement for explicit
//! lifecycle states with guarded transitions.
//!
//! ## Design Goals
//!
//! - **Type Safety**: Leverage Rust's type system to prevent invalid states
//! - **Explicit Transitions**: All valid transitions defined in one place
//! - **Business Rule Validation**: Runtime checks for ledger timing, authorization
//! - **Audit Trail**: Event emissions for all state changes
//! - **Zero Overhead**: Inlined validators with no cross-contract calls
//!
//! ## Supported Lifecycles
//!
//! - **AidRecord**: `Pending → Settled | Refunded`
//! - **EscrowRecord**: `Active → Released | Refunded`
//! - **Proposal**: `Pending → Executed`
//!
//! ## Usage Example
//!
//! ```ignore
//! use shared::lifecycle::{AidStateMachine, AidTransitionContext};
//!
//! let ctx = AidTransitionContext {
//!     current_ledger: env.ledger().sequence(),
//!     expiry_ledger: record.expiry_ledger,
//!     caller: caller.clone(),
//!     recipient: record.recipient.clone(),
//! };
//!
//! // Validate transition before mutating state
//! AidStateMachine::can_transition(
//!     record.status,
//!     AidStatus::Settled,
//!     &ctx,
//! )?;
//!
//! // Safe to update state
//! record.status = AidStatus::Settled;
//! ```

use soroban_sdk::{Address, Env, Symbol};

use crate::errors::Error as SharedError;

// Type alias for ergonomics
type Error = SharedError;

// Re-export status enums from their respective modules for convenience
pub use crate::payments::EscrowState;

// ===========================================================================
// State Machine Trait (Generic Interface)
// ===========================================================================

/// Generic state machine trait for lifecycle validation.
///
/// Implementers define:
/// - `State`: The enum representing possible states
/// - `TransitionContext`: Business rule context (ledger, caller, etc.)
/// - `Error`: Error type for invalid transitions
pub trait StateMachine {
    /// The state enum type (e.g., `AidStatus`, `EscrowState`)
    type State: Copy + Clone + Eq + PartialEq;

    /// Context needed to validate business rules for a transition
    type TransitionContext;

    /// Error type returned when a transition is invalid
    type Error;

    /// Check if a transition from `current` to `next` is valid given `context`.
    ///
    /// Returns `Ok(())` if the transition is allowed, or an appropriate error
    /// describing why the transition is invalid.
    fn can_transition(
        current: Self::State,
        next: Self::State,
        context: &Self::TransitionContext,
    ) -> Result<(), Self::Error>;

    /// Execute a validated state transition.
    ///
    /// This is a convenience wrapper that calls `can_transition` and returns
    /// the new state on success.
    fn transition(
        current: Self::State,
        next: Self::State,
        context: &Self::TransitionContext,
    ) -> Result<Self::State, Self::Error> {
        Self::can_transition(current, next, context)?;
        Ok(next)
    }
}

// ===========================================================================
// Aid Lifecycle State Machine
// ===========================================================================

/// State machine for `AidRecord` lifecycle.
///
/// Valid transitions:
/// - `Pending → Settled` (recipient claims before expiry)
/// - `Pending → Refunded` (donor refunds after expiry)
///
/// All other transitions are invalid and will return an error.
pub struct AidStateMachine;

/// Context required to validate an Aid state transition.
pub struct AidTransitionContext {
    /// Current ledger sequence number
    pub current_ledger: u32,
    /// Ledger sequence at which the aid expires
    pub expiry_ledger: u32,
    /// Address initiating the transition
    pub caller: Address,
    /// Intended recipient of the aid
    pub recipient: Address,
}

/// Aid status enum (re-declared here for the state machine; matches aid-contract)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AidStatus {
    Pending,
    Settled,
    Refunded,
}

impl StateMachine for AidStateMachine {
    type State = AidStatus;
    type TransitionContext = AidTransitionContext;
    type Error = Error;

    fn can_transition(
        current: AidStatus,
        next: AidStatus,
        ctx: &AidTransitionContext,
    ) -> Result<(), Error> {
        match (current, next) {
            // Pending → Settled: Recipient claims before expiry
            (AidStatus::Pending, AidStatus::Settled) => {
                if ctx.current_ledger > ctx.expiry_ledger {
                    return Err(Error::Expired);
                }
                if ctx.caller != ctx.recipient {
                    return Err(Error::Unauthorized);
                }
                Ok(())
            }

            // Pending → Refunded: Donor refunds after expiry
            (AidStatus::Pending, AidStatus::Refunded) => {
                if ctx.current_ledger <= ctx.expiry_ledger {
                    return Err(Error::AidNotExpiredYet);
                }
                Ok(())
            }

            // Idempotency: attempting to re-apply the same terminal state
            (AidStatus::Settled, AidStatus::Settled) => Err(Error::AlreadyClaimed),
            (AidStatus::Refunded, AidStatus::Refunded) => Err(Error::AidAlreadyRefunded),

            // Cross-transitions from terminal states
            (AidStatus::Settled, _) => Err(Error::AlreadyClaimed),
            (AidStatus::Refunded, _) => Err(Error::AidAlreadyRefunded),

            // Any other transition is invalid
            _ => Err(Error::InvalidTransition),
        }
    }
}

// ===========================================================================
// Escrow Lifecycle State Machine
// ===========================================================================

/// State machine for `EscrowRecord` lifecycle.
///
/// Valid transitions:
/// - `Active → Released` (authorized release before/at expiry)
/// - `Active → Refunded` (authorized refund, typically after expiry)
///
/// All other transitions are invalid.
pub struct EscrowStateMachine;

/// Context required to validate an Escrow state transition.
pub struct EscrowTransitionContext {
    /// Current ledger sequence number
    pub current_ledger: u32,
    /// Ledger sequence at which the escrow expires
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
            // Active → Released: Release before or at expiry
            (EscrowState::Active, EscrowState::Released) => {
                if ctx.current_ledger > ctx.expiry_ledger {
                    return Err(Error::PaymentEscrowExpired);
                }
                Ok(())
            }

            // Active → Refunded: Refund allowed any time (business logic controls authorization)
            (EscrowState::Active, EscrowState::Refunded) => Ok(()),

            // Idempotency: attempting to re-apply the same terminal state
            (EscrowState::Released, EscrowState::Released) => Err(Error::PaymentEscrowAlreadyReleased),
            (EscrowState::Refunded, EscrowState::Refunded) => Err(Error::PaymentEscrowAlreadyRefunded),

            // Cross-transitions from terminal states
            (EscrowState::Released, _) => Err(Error::PaymentEscrowAlreadyReleased),
            (EscrowState::Refunded, _) => Err(Error::PaymentEscrowAlreadyRefunded),

            // Any other transition is invalid
            _ => Err(Error::InvalidTransition),
        }
    }
}

// ===========================================================================
// Proposal Lifecycle State Machine
// ===========================================================================

/// State machine for `Proposal` lifecycle in governance.
///
/// Valid transitions:
/// - `Pending → Executed` (when approval threshold is met)
///
/// All other transitions are invalid.
pub struct ProposalStateMachine;

/// Context required to validate a Proposal state transition.
pub struct ProposalTransitionContext {
    /// Number of approvals collected
    pub approval_count: u32,
    /// Minimum approvals required to execute
    pub threshold: u32,
}

/// Proposal status enum (re-declared here; matches governance-contract)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    Pending,
    Executed,
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
            // Pending → Executed: Must meet approval threshold
            (ProposalStatus::Pending, ProposalStatus::Executed) => {
                if ctx.approval_count < ctx.threshold {
                    return Err(Error::BelowThreshold);
                }
                Ok(())
            }

            // Idempotency: attempting to re-execute
            (ProposalStatus::Executed, ProposalStatus::Executed) => Err(Error::AlreadyExecuted),

            // Reverse transition not allowed
            (ProposalStatus::Executed, ProposalStatus::Pending) => Err(Error::AlreadyExecuted),

            // Any other transition is invalid
            _ => Err(Error::InvalidTransition),
        }
    }
}

// ===========================================================================
// Event Emission for State Transitions
// ===========================================================================

/// Emit a structured event for any state transition.
///
/// This provides a unified audit trail across all lifecycle state machines.
///
/// # Arguments
/// * `env` - Soroban environment
/// * `record_type` - Symbol identifying the record type ("aid", "escrow", "proposal")
/// * `record_id` - Unique identifier of the record
/// * `from_state` - Symbol representing the previous state
/// * `to_state` - Symbol representing the new state
/// * `actor` - Address that triggered the transition
/// * `timestamp` - Ledger timestamp of the transition
pub fn emit_state_transition(
    env: &Env,
    record_type: Symbol,
    record_id: u64,
    from_state: Symbol,
    to_state: Symbol,
    actor: &Address,
    timestamp: u64,
) {
    use soroban_sdk::symbol_short;

    env.events().publish(
        (symbol_short!("state"), symbol_short!("trans")),
        (
            record_type,
            record_id,
            from_state,
            to_state,
            actor.clone(),
            timestamp,
        ),
    );
}

// ===========================================================================
// Helper Conversions
// ===========================================================================

/// Convert `AidStatus` to a Symbol for event emission.
pub fn aid_status_to_symbol(_env: &Env, status: AidStatus) -> Symbol {
    use soroban_sdk::symbol_short;
    match status {
        AidStatus::Pending => symbol_short!("pending"),
        AidStatus::Settled => symbol_short!("settled"),
        AidStatus::Refunded => symbol_short!("refunded"),
    }
}

/// Convert `EscrowState` to a Symbol for event emission.
pub fn escrow_state_to_symbol(_env: &Env, state: EscrowState) -> Symbol {
    use soroban_sdk::symbol_short;
    match state {
        EscrowState::Active => symbol_short!("active"),
        EscrowState::Released => symbol_short!("released"),
        EscrowState::Refunded => symbol_short!("refunded"),
    }
}

/// Convert `ProposalStatus` to a Symbol for event emission.
pub fn proposal_status_to_symbol(_env: &Env, status: ProposalStatus) -> Symbol {
    use soroban_sdk::symbol_short;
    match status {
        ProposalStatus::Pending => symbol_short!("pending"),
        ProposalStatus::Executed => symbol_short!("executed"),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    // -----------------------------------------------------------------------
    // Aid State Machine Tests
    // -----------------------------------------------------------------------

    #[test]
    fn aid_pending_to_settled_valid() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let ctx = AidTransitionContext {
            current_ledger: 100,
            expiry_ledger: 200,
            caller: caller.clone(),
            recipient: caller.clone(),
        };

        let result = AidStateMachine::can_transition(
            AidStatus::Pending,
            AidStatus::Settled,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn aid_pending_to_settled_expired() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let ctx = AidTransitionContext {
            current_ledger: 300,
            expiry_ledger: 200,
            caller: caller.clone(),
            recipient: caller.clone(),
        };

        let result = AidStateMachine::can_transition(
            AidStatus::Pending,
            AidStatus::Settled,
            &ctx,
        );
        assert_eq!(result, Err(Error::Expired));
    }

    #[test]
    fn aid_pending_to_settled_unauthorized() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let recipient = Address::generate(&env);
        let ctx = AidTransitionContext {
            current_ledger: 100,
            expiry_ledger: 200,
            caller,
            recipient,
        };

        let result = AidStateMachine::can_transition(
            AidStatus::Pending,
            AidStatus::Settled,
            &ctx,
        );
        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn aid_pending_to_refunded_valid() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let ctx = AidTransitionContext {
            current_ledger: 300,
            expiry_ledger: 200,
            caller: caller.clone(),
            recipient: caller.clone(),
        };

        let result = AidStateMachine::can_transition(
            AidStatus::Pending,
            AidStatus::Refunded,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn aid_pending_to_refunded_not_expired() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let ctx = AidTransitionContext {
            current_ledger: 100,
            expiry_ledger: 200,
            caller: caller.clone(),
            recipient: caller.clone(),
        };

        let result = AidStateMachine::can_transition(
            AidStatus::Pending,
            AidStatus::Refunded,
            &ctx,
        );
        assert_eq!(result, Err(Error::AidNotExpiredYet));
    }

    #[test]
    fn aid_settled_to_refunded_invalid() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let ctx = AidTransitionContext {
            current_ledger: 300,
            expiry_ledger: 200,
            caller: caller.clone(),
            recipient: caller.clone(),
        };

        let result = AidStateMachine::can_transition(
            AidStatus::Settled,
            AidStatus::Refunded,
            &ctx,
        );
        assert_eq!(result, Err(Error::AlreadyClaimed));
    }

    #[test]
    fn aid_refunded_to_settled_invalid() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let ctx = AidTransitionContext {
            current_ledger: 100,
            expiry_ledger: 200,
            caller: caller.clone(),
            recipient: caller.clone(),
        };

        let result = AidStateMachine::can_transition(
            AidStatus::Refunded,
            AidStatus::Settled,
            &ctx,
        );
        assert_eq!(result, Err(Error::AidAlreadyRefunded));
    }

    #[test]
    fn aid_settled_to_settled_idempotent_rejected() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let ctx = AidTransitionContext {
            current_ledger: 100,
            expiry_ledger: 200,
            caller: caller.clone(),
            recipient: caller.clone(),
        };

        let result = AidStateMachine::can_transition(
            AidStatus::Settled,
            AidStatus::Settled,
            &ctx,
        );
        assert_eq!(result, Err(Error::AlreadyClaimed));
    }

    #[test]
    fn aid_refunded_to_refunded_idempotent_rejected() {
        let env = Env::default();
        let caller = Address::generate(&env);
        let ctx = AidTransitionContext {
            current_ledger: 300,
            expiry_ledger: 200,
            caller: caller.clone(),
            recipient: caller.clone(),
        };

        let result = AidStateMachine::can_transition(
            AidStatus::Refunded,
            AidStatus::Refunded,
            &ctx,
        );
        assert_eq!(result, Err(Error::AidAlreadyRefunded));
    }

    // -----------------------------------------------------------------------
    // Escrow State Machine Tests
    // -----------------------------------------------------------------------

    #[test]
    fn escrow_active_to_released_valid() {
        let ctx = EscrowTransitionContext {
            current_ledger: 100,
            expiry_ledger: 200,
        };

        let result = EscrowStateMachine::can_transition(
            EscrowState::Active,
            EscrowState::Released,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn escrow_active_to_released_expired() {
        let ctx = EscrowTransitionContext {
            current_ledger: 300,
            expiry_ledger: 200,
        };

        let result = EscrowStateMachine::can_transition(
            EscrowState::Active,
            EscrowState::Released,
            &ctx,
        );
        assert_eq!(result, Err(Error::PaymentEscrowExpired));
    }

    #[test]
    fn escrow_active_to_refunded_valid() {
        let ctx = EscrowTransitionContext {
            current_ledger: 300,
            expiry_ledger: 200,
        };

        let result = EscrowStateMachine::can_transition(
            EscrowState::Active,
            EscrowState::Refunded,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn escrow_released_to_refunded_invalid() {
        let ctx = EscrowTransitionContext {
            current_ledger: 300,
            expiry_ledger: 200,
        };

        let result = EscrowStateMachine::can_transition(
            EscrowState::Released,
            EscrowState::Refunded,
            &ctx,
        );
        assert_eq!(result, Err(Error::PaymentEscrowAlreadyReleased));
    }

    #[test]
    fn escrow_refunded_to_released_invalid() {
        let ctx = EscrowTransitionContext {
            current_ledger: 100,
            expiry_ledger: 200,
        };

        let result = EscrowStateMachine::can_transition(
            EscrowState::Refunded,
            EscrowState::Released,
            &ctx,
        );
        assert_eq!(result, Err(Error::PaymentEscrowAlreadyRefunded));
    }

    #[test]
    fn escrow_released_to_released_idempotent_rejected() {
        let ctx = EscrowTransitionContext {
            current_ledger: 100,
            expiry_ledger: 200,
        };

        let result = EscrowStateMachine::can_transition(
            EscrowState::Released,
            EscrowState::Released,
            &ctx,
        );
        assert_eq!(result, Err(Error::PaymentEscrowAlreadyReleased));
    }

    #[test]
    fn escrow_refunded_to_refunded_idempotent_rejected() {
        let ctx = EscrowTransitionContext {
            current_ledger: 300,
            expiry_ledger: 200,
        };

        let result = EscrowStateMachine::can_transition(
            EscrowState::Refunded,
            EscrowState::Refunded,
            &ctx,
        );
        assert_eq!(result, Err(Error::PaymentEscrowAlreadyRefunded));
    }

    // -----------------------------------------------------------------------
    // Proposal State Machine Tests
    // -----------------------------------------------------------------------

    #[test]
    fn proposal_pending_to_executed_valid() {
        let ctx = ProposalTransitionContext {
            approval_count: 3,
            threshold: 2,
        };

        let result = ProposalStateMachine::can_transition(
            ProposalStatus::Pending,
            ProposalStatus::Executed,
            &ctx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn proposal_pending_to_executed_below_threshold() {
        let ctx = ProposalTransitionContext {
            approval_count: 1,
            threshold: 2,
        };

        let result = ProposalStateMachine::can_transition(
            ProposalStatus::Pending,
            ProposalStatus::Executed,
            &ctx,
        );
        assert_eq!(result, Err(Error::BelowThreshold));
    }

    #[test]
    fn proposal_executed_to_pending_invalid() {
        let ctx = ProposalTransitionContext {
            approval_count: 3,
            threshold: 2,
        };

        let result = ProposalStateMachine::can_transition(
            ProposalStatus::Executed,
            ProposalStatus::Pending,
            &ctx,
        );
        assert_eq!(result, Err(Error::AlreadyExecuted));
    }

    #[test]
    fn proposal_executed_to_executed_idempotent_rejected() {
        let ctx = ProposalTransitionContext {
            approval_count: 3,
            threshold: 2,
        };

        let result = ProposalStateMachine::can_transition(
            ProposalStatus::Executed,
            ProposalStatus::Executed,
            &ctx,
        );
        assert_eq!(result, Err(Error::AlreadyExecuted));
    }
}
