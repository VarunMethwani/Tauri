//! Provider-neutral foundations for the Agent Harness.
//!
//! The crate deliberately ships without credentials or a network-backed model
//! integration. [`OfflineProvider`] is an honest deterministic demo that makes
//! the runtime, streaming, and cancellation behavior usable while a client
//! supplies its own [`Provider`] implementation.

mod permissions;
mod runtime;
mod store;
mod types;

pub use permissions::{evaluate_permission, Operation, PermissionDecision, PermissionEvaluator};
pub use runtime::{
    start_offline_turn, AgentRuntime, CancellationHandle, OfflineProvider, Provider,
    ProviderError, ProviderFuture, TurnRequest, DEFAULT_EVENT_CAPACITY,
};
pub use store::{HarnessError, Result, SessionStore};
pub use types::{AgentEvent, Message, PermissionMode, Session};

