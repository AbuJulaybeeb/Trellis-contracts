//! Testing & Simulation Module for Soroban contracts
//!
//! Provides comprehensive test harnesses, mocks, fuzzing helpers, and simulation tools
//! for all contracts in the trellis-contracts repository.

#![no_std]

pub mod examples;
pub mod fuzzing;
pub mod helpers;
pub mod mocks;
pub mod sandbox;
pub mod simulation;
pub mod upgrade;

pub use fuzzing::*;
pub use helpers::*;
pub use mocks::*;
pub use sandbox::*;
pub use simulation::*;
pub use upgrade::*;
