//! RFC-049 slice 2: Form Mode's Undo and Redo, reached by two buttons and by
//! Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y (`assets/shortcuts.js`, which sends them
//! only while Form Mode is shown and focus is not in a text field).
//!
//! Slice 1 (`bekoedit-core`) owns the history and its one guard. This only
//! calls `AppState::undo_form` / `redo_form` and reports a refusal, as task
//! 056 does for a refused edit: never silently, and never with document text.

use dioxus::prelude::*;

use bekoedit_core::{AppState, SessionError, StoreError};

use crate::components::toast::{Toast, ToastKind, push_toast};
use crate::i18n::{Lang, tr};
use crate::state::now_ms;

use super::{edit_refused_toast_key, edit_refused_trace_details};

/// Runs one Undo (`undo`) or Redo through the store and reports a refusal.
/// An empty history is `Ok(false)`: no notice, the button is disabled and
/// the shortcut simply does nothing.
pub(crate) fn run_history_step(
    mut state: Signal<AppState>,
    mut toasts: Signal<Vec<Toast>>,
    lang: Lang,
    undo: bool,
) {
    let result = if undo {
        state.write().undo_form(now_ms())
    } else {
        state.write().redo_form(now_ms())
    };
    if let Some((details, toast_key)) = history_outcome(&result) {
        crate::bridge::trace("form.history_refused", details);
        push_toast(&mut toasts, ToastKind::Warning, tr(lang, toast_key));
    }
}

/// What `run_history_step` does with the store's result: the trace details and
/// the toast key, or `None` when nothing needs saying (it applied, or there
/// was nothing to undo). Pure, so each case is tested without a live `Signal`.
pub(super) fn history_outcome(result: &Result<bool, StoreError>) -> Option<(String, &'static str)> {
    let error = result.as_ref().err()?;
    Some(match error {
        StoreError::Session(SessionError::FormHistoryStale) => {
            ("kind=form_history_stale".to_string(), "form.history.stale")
        }
        other => (
            edit_refused_trace_details(other),
            edit_refused_toast_key(other),
        ),
    })
}

/// The Undo and Redo buttons, at the top of the Form view.
#[component]
pub(super) fn FormHistoryBar(lang: Lang) -> Element {
    let state = use_context::<Signal<AppState>>();
    let toasts = use_context::<Signal<Vec<Toast>>>();
    let (can_undo, can_redo) = state
        .read()
        .session
        .as_ref()
        .map_or((false, false), |s| (s.can_undo_form(), s.can_redo_form()));
    rsx! {
        div {
            class: "form-history",
            role: "toolbar",
            "aria-label": tr(lang, "form.history.label"),
            button {
                id: "form-undo",
                class: "form-history-btn form-history-undo",
                disabled: !can_undo,
                title: tr(lang, "form.undo"),
                "aria-label": tr(lang, "form.undo"),
                onclick: move |_| run_history_step(state, toasts, lang, true),
                {tr(lang, "form.undo")}
            }
            button {
                id: "form-redo",
                class: "form-history-btn form-history-redo",
                disabled: !can_redo,
                title: tr(lang, "form.redo"),
                "aria-label": tr(lang, "form.redo"),
                onclick: move |_| run_history_step(state, toasts, lang, false),
                {tr(lang, "form.redo")}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stale_history_raises_its_own_notice_and_trace() {
        let result = Err(StoreError::Session(SessionError::FormHistoryStale));
        assert_eq!(
            history_outcome(&result),
            Some(("kind=form_history_stale".to_string(), "form.history.stale"))
        );
    }

    #[test]
    fn an_applied_step_and_an_empty_history_say_nothing() {
        assert_eq!(history_outcome(&Ok(true)), None);
        assert_eq!(history_outcome(&Ok(false)), None);
    }

    #[test]
    fn a_pending_conflict_is_reported_through_the_edit_refusal_path() {
        let (details, key) = history_outcome(&Err(StoreError::ConflictPending)).unwrap();
        assert_eq!(details, "kind=conflict_pending");
        assert_eq!(key, "form.edit_refused.unsupported");
    }

    /// The notice must exist in both languages and carry no document text.
    #[test]
    fn the_notice_and_the_labels_exist_in_both_languages() {
        for lang in [Lang::En, Lang::Ja] {
            for key in [
                "form.history.stale",
                "form.history.label",
                "form.undo",
                "form.redo",
            ] {
                assert!(!tr(lang, key).is_empty(), "{lang:?} {key}");
            }
        }
    }
}
