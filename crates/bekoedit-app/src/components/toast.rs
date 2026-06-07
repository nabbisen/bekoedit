//! Toast notifications (RFC-023): brief non-blocking feedback for saves,
//! errors, and background events that don't warrant a modal dialog.

use std::time::Duration;

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: String,
}

/// Push a toast from any component: `push_toast(&mut toasts, kind, message)`.
pub fn push_toast(toasts: &mut Signal<Vec<Toast>>, kind: ToastKind, message: impl Into<String>) {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let toast = Toast {
        id,
        kind,
        message: message.into(),
    };
    toasts.write().push(toast);
    // Dismiss after 4 s using a detached async task.
    let id_copy = id;
    let mut t = *toasts;
    spawn(async move {
        tokio::time::sleep(Duration::from_secs(4)).await;
        t.write().retain(|toast| toast.id != id_copy);
    });
}

/// Renders the live toast region (ARIA role="status" for polite
/// announcements, RFC-021/023).
#[component]
pub fn ToastLayer() -> Element {
    let toasts = use_context::<Signal<Vec<Toast>>>();
    let items = toasts.read().clone();
    if items.is_empty() {
        return rsx! {};
    }
    rsx! {
        div {
            class: "toast-layer",
            role: "status",
            aria_live: "polite",
            aria_atomic: "false",
            for toast in items {
                div {
                    key: "{toast.id}",
                    class: "toast toast-{toast.kind:?}".to_lowercase(),
                    "{toast.message}"
                }
            }
        }
    }
}
