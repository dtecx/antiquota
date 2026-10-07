//! Owns pure domain types, state transitions, and adapter contracts.
pub mod accounts;
pub mod routing;
pub use routing::*;
pub mod command;
pub mod event;
pub mod hooks;
pub mod mentions;
pub mod policy;
pub mod ports;
pub mod session;
pub mod settings;
pub mod transition;
pub use accounts::*;
pub use command::*;
pub use event::*;
pub use policy::{Decision, Permissions, Verdict};
pub use ports::*;
pub use session::*;
pub use settings::*;
pub use transition::*;
pub const DEFAULT_MODEL: &str = "gemini-3.8-flash-high";
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[cfg(test)]
mod event_tests;

mod event_codec;
