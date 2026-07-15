//! Toast notifications (RFC-023): brief non-blocking feedback for saves,
//! errors, and background events that don't warrant a modal dialog.

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
}

pub fn dismiss_toast(toasts: &mut Signal<Vec<Toast>>, id: u64) {
    toasts.write().retain(|toast| toast.id != id);
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
                ToastItem {
                    key: "{toast.id}",
                    toast,
                }
            }
        }
    }
}

#[component]
fn ToastItem(toast: Toast) -> Element {
    let mut toasts = use_context::<Signal<Vec<Toast>>>();
    let lang = *use_context::<Signal<crate::i18n::Lang>>().read();
    let id = toast.id;
    use_future(move || async move {
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        dismiss_toast(&mut toasts, id);
    });
    rsx! {
        div {
            class: "toast toast-{toast.kind:?}".to_lowercase(),
            span { class: "toast-message", "{toast.message}" }
            button {
                class: "toast-close",
                aria_label: crate::i18n::tr(lang, "toast.dismiss"),
                title: crate::i18n::tr(lang, "toast.dismiss"),
                onclick: move |_| dismiss_toast(&mut toasts, id),
                "×"
            }
        }
    }
}
