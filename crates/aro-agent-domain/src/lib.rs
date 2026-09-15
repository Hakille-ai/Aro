//! Provider-neutral domain contracts for ARO's durable agent runtime.
//!
//! This crate deliberately has no database, HTTP, worker, model-provider, or tool-executor
//! dependency. State transitions are pure: callers provide identity and time, persist the
//! returned aggregate and event atomically, and own idempotent command handling.

#![forbid(unsafe_code)]

pub mod aggregate;
pub mod canonical;
pub mod command;
pub mod error;
pub mod event;
pub mod ids;
pub mod reducer;
pub mod spec;
pub mod state;

pub use aggregate::*;
pub use canonical::*;
pub use command::*;
pub use error::*;
pub use event::*;
pub use ids::*;
pub use reducer::*;
pub use spec::*;
pub use state::*;
