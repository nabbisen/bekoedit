//! Link clicks (task 032): the app's only route from a click on a link to an
//! OS opener.
//!
//! `link_guard.js` cancels every click on an `<a>` before Dioxus's own
//! interception can send its raw `href` to `webbrowser::open`, switches that
//! interception off as a second layer, and forwards the `href` here. `bekoedit_core::decide_link_click` decides; this module
//! does what it says. `webbrowser::open` is called nowhere else in bekoedit,
//! and only with an `ExternalUrl`, which only that function can build, from
//! an `http:`, `https:` or `mailto:` destination.

use bekoedit_core::{AppState, ExternalUrl, LinkAction, LinkRefusal, decide_link_click};
use dioxus::prelude::*;

use crate::components::toast::{Toast, ToastKind, push_toast};
use crate::i18n::{Lang, tr};

pub const LINK_GUARD_JS: &str = include_str!("link_guard.js");

/// What the script sends. Anything else it might send is ignored, so an
/// unexpected message cannot end the loop.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum GuardMessage {
    /// A click on a link, with its raw `href`.
    Click { href: String },
    /// The script noticed something wrong with itself, for example that the
    /// interpreter's own link route could not be switched off.
    Trace { detail: String },
}

pub fn parse_guard_message(value: serde_json::Value) -> Option<GuardMessage> {
    serde_json::from_value(value).ok()
}

/// What the app does for a decided link click.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkEffect {
    /// Hand the URL to the system browser or mail client.
    Open(ExternalUrl),
    /// Nothing outside the page happens, and nothing is owed to the reader.
    Nothing,
    /// The link was not followed. The notice's i18n key says why.
    Notice(&'static str),
}

pub fn link_effect(action: LinkAction) -> LinkEffect {
    match action {
        LinkAction::OpenExternal(url) => LinkEffect::Open(url),
        // Headings carry no ids, so there is nothing to scroll to yet; the
        // click stays in the page.
        LinkAction::InPage(_) => LinkEffect::Nothing,
        // Resolves to a document inside the workspace. Opening it from
        // Preview needs the source-focus handoff (RFC-042), so it is not
        // followed yet, and the reader is told.
        LinkAction::OpenDocument(_) => LinkEffect::Notice("link.not_followed"),
        LinkAction::Refuse(reason) => LinkEffect::Notice(match reason {
            LinkRefusal::NoTarget => "link.no_target",
            LinkRefusal::UnsupportedScheme => "link.unsupported_scheme",
            LinkRefusal::NetworkPath => "link.network_path",
            LinkRefusal::AbsolutePath => "link.absolute_path",
            LinkRefusal::InvalidPath => "link.invalid_path",
            LinkRefusal::NoBase => "link.no_base",
            LinkRefusal::OutsideWorkspace => "link.outside_workspace",
            LinkRefusal::NotFound => "link.not_found",
            LinkRefusal::NotADocument => "link.not_a_document",
        }),
    }
}

/// Starts the guard for the life of the app. Call once, from the root.
pub fn use_link_guard() {
    let state = use_context::<Signal<AppState>>();
    let lang = use_context::<Signal<Lang>>();
    let mut toasts = use_context::<Signal<Vec<Toast>>>();
    use_future(move || async move {
        let mut eval = document::eval(LINK_GUARD_JS);
        // A receive error ends the loop. The script still cancels every
        // link click, so a dead loop fails closed: links do nothing.
        while let Ok(value) = eval.recv::<serde_json::Value>().await {
            let href = match parse_guard_message(value) {
                Some(GuardMessage::Click { href }) => href,
                Some(GuardMessage::Trace { detail }) => {
                    eprintln!("bekoedit: link guard: {detail}");
                    continue;
                }
                None => continue,
            };
            let (document, root) = {
                let state = state.read();
                (
                    state.session.as_ref().map(|session| session.path.clone()),
                    state
                        .workspace
                        .as_ref()
                        .map(|space| space.root_path.clone()),
                )
            };
            let action = decide_link_click(&href, document.as_deref(), root.as_deref());
            let lang = *lang.read();
            match link_effect(action) {
                LinkEffect::Open(url) => {
                    if webbrowser::open(url.as_str()).is_err() {
                        push_toast(
                            &mut toasts,
                            ToastKind::Warning,
                            tr(lang, "link.open_failed"),
                        );
                    }
                }
                LinkEffect::Nothing => {}
                LinkEffect::Notice(key) => push_toast(&mut toasts, ToastKind::Info, tr(lang, key)),
            }
        }
    });
}

#[cfg(test)]
mod tests;
