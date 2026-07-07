#![allow(dead_code)]
//! Maps `StoreError` and `FileOpError` to user-facing i18n message keys.
//!
//! The UI calls `store_error_key(err)` and passes the result to `tr(lang, key)`
//! rather than displaying raw Rust error strings to users.

use bekoedit_core::StoreError;
use bekoedit_fs::FileOpError;

/// Returns the i18n message key for a `StoreError`.
pub fn store_error_key(err: &StoreError) -> &'static str {
    match err {
        StoreError::NoWorkspace => "error.no_workspace",
        StoreError::NoDocument => "error.no_document",
        StoreError::ConflictPending => "error.conflict_pending",
        StoreError::DocumentDirty => "error.document_dirty",
        StoreError::SaveFailed(msg) => classify_save_error(msg),
        StoreError::FileOp(e) => file_op_error_key(e),
        // Workspace and session errors fall through to a generic save-failed message
        // rather than exposing internal Rust types to users.
        _ => "error.save_failed",
    }
}

fn classify_save_error(msg: &str) -> &'static str {
    let lower = msg.to_lowercase();
    if lower.contains("permission") || lower.contains("access") || lower.contains("denied") {
        "error.save_failed_permission"
    } else if lower.contains("no space") || lower.contains("disk full") || lower.contains("storage")
    {
        "error.save_failed_disk_full"
    } else {
        "error.save_failed"
    }
}

fn file_op_error_key(err: &FileOpError) -> &'static str {
    match err {
        FileOpError::Path(_) => "error.path_traversal",
        FileOpError::AlreadyExists => "error.file_already_exists",
        FileOpError::NotFound => "error.file_not_found",
        FileOpError::Io(msg) | FileOpError::TrashFailed(msg) => classify_save_error(msg),
    }
}
