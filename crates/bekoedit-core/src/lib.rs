//! Document sessions, canonical source model, save lifecycle, conflict
//! detection, and the application state store for bekoedit.
//!
//! Implements:
//! - RFC-006: document session and canonical source model
//! - RFC-007: save, autosave, atomic write, recovery (policy side)
//! - RFC-008: dirty state and external-modification conflicts
//! - RFC-009: application state store and command/event ordering
//!
//! The canonical text lives here, in Rust memory, and is mutated only
//! through validated text snapshots (Text Mode) or resolved source patches
//! (Form Mode) — never by the UI directly.

pub mod conflict;
pub mod save;
pub mod session;
pub mod store;
mod store_exports;
mod store_file_ops;
mod store_history;
mod store_sections;
mod store_templates;

pub use conflict::{ConflictResolution, ConflictState};
pub use save::{AutosaveScheduler, SaveState};
pub use session::{DocumentSession, SessionError};
pub use store::{AppState, StoreError};

#[cfg(test)]
mod tests;
