//! RFC-047 slice 2: the one sentence a discarded queued command becomes for
//! the user -- what did not happen, then why -- and the grouping that keeps
//! several discards sharing one event to one message (§4).

use std::borrow::Cow;
use std::path::Path;

use bekoedit_ui_contract::EditorMode;

use crate::i18n::{Lang, tr};

use super::{DiscardReason, QueueDiscard, SourceCommand};

/// The file name a path-carrying command names, never the full path
/// (RFC-047 slice 2 handoff §3.1): a user reads "notes.md", not where it
/// lives on disk.
fn file_label(path: &Path) -> Cow<'_, str> {
    path.file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| path.to_string_lossy())
}

/// What did not happen. Exhaustive over `SourceCommand`, so a new variant
/// must be classified here before it compiles (handoff §3.1).
fn action_phrase(command: &SourceCommand, lang: Lang) -> String {
    match command {
        SourceCommand::SwitchMode(EditorMode::Text) => {
            tr(lang, "queue.action.switch_mode.text").to_string()
        }
        SourceCommand::SwitchMode(EditorMode::Form) => {
            tr(lang, "queue.action.switch_mode.form").to_string()
        }
        SourceCommand::SwitchMode(EditorMode::Preview) => {
            tr(lang, "queue.action.switch_mode.preview").to_string()
        }
        SourceCommand::SwitchMode(EditorMode::Split) => {
            tr(lang, "queue.action.switch_mode.split").to_string()
        }
        SourceCommand::OpenDocument(path) => {
            tr(lang, "queue.action.open_document").replacen("{}", &file_label(path), 1)
        }
        SourceCommand::SaveNow => tr(lang, "queue.action.save_now").to_string(),
        SourceCommand::SaveAs(path) => {
            tr(lang, "queue.action.save_as").replacen("{}", &file_label(path), 1)
        }
        SourceCommand::NewUntitled => tr(lang, "queue.action.new_untitled").to_string(),
        SourceCommand::OpenWorkspace(_) => tr(lang, "queue.action.open_workspace").to_string(),
        SourceCommand::CloseWorkspace => tr(lang, "queue.action.close_workspace").to_string(),
        SourceCommand::OpenSettings => tr(lang, "queue.action.open_settings").to_string(),
        SourceCommand::MoveSectionUp(_) => tr(lang, "queue.action.move_section_up").to_string(),
        SourceCommand::MoveSectionDown(_) => tr(lang, "queue.action.move_section_down").to_string(),
        SourceCommand::RestoreHistory(_) => tr(lang, "queue.action.restore_history").to_string(),
    }
}

/// More than one discard shared a reason: named by count, not by listing
/// each action (RFC-047 §5.5's grouping) -- composing a list would assume
/// English's conjunctions, which Japanese does not share (handoff §4).
fn many_action_phrase(count: usize, lang: Lang) -> String {
    tr(lang, "queue.action.many").replacen("{}", &count.to_string(), 1)
}

/// Why it did not happen. `None` for `Shutdown`: the window is closing, and a
/// toast that cannot be read is noise in the log, not information for a user
/// (RFC-047 §5.5, amended). Exhaustive over `DiscardReason`.
fn reason_clause(reason: DiscardReason, lang: Lang) -> Option<&'static str> {
    match reason {
        DiscardReason::Overflow | DiscardReason::Expired => Some(tr(lang, "queue.reason.busy")),
        DiscardReason::DocumentChanged => Some(tr(lang, "queue.reason.document_changed")),
        DiscardReason::EditorUnavailable | DiscardReason::RelayLost => {
            Some(tr(lang, "queue.reason.unresponsive"))
        }
        DiscardReason::Shutdown => None,
    }
}

/// One message per reason represented in `discards`, in the order each
/// reason was first seen -- report once per reason (RFC-047 §5.5's
/// grouping), never once per discard. `Shutdown` discards produce nothing,
/// and produce no empty group either.
pub fn discard_messages(discards: &[QueueDiscard], lang: Lang) -> Vec<String> {
    let mut groups: Vec<(DiscardReason, Vec<&SourceCommand>)> = Vec::new();
    for discard in discards {
        if reason_clause(discard.reason, lang).is_none() {
            continue;
        }
        match groups
            .iter_mut()
            .find(|(reason, _)| *reason == discard.reason)
        {
            Some((_, commands)) => commands.push(&discard.command),
            None => groups.push((discard.reason, vec![&discard.command])),
        }
    }
    groups
        .into_iter()
        .map(|(reason, commands)| {
            let reason_text = reason_clause(reason, lang).expect("filtered above");
            let action = match commands.as_slice() {
                [only] => action_phrase(only, lang),
                many => many_action_phrase(many.len(), lang),
            };
            compose(&action, reason_text, lang)
        })
        .collect()
}

/// Joins an action and a reason into one sentence, in whichever order and
/// punctuation `queue.template` gives for `lang`: EN states the effect then
/// the cause; JA states the cause then the effect, as one sentence (RFC-047
/// slice 2 review §3) -- word order and closing punctuation are translatable
/// data here, not a Rust format string one language was written to fit.
fn compose(action: &str, reason: &str, lang: Lang) -> String {
    tr(lang, "queue.template")
        .replacen("{action}", action, 1)
        .replacen("{reason}", reason, 1)
}

#[cfg(test)]
mod tests;
