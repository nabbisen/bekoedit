//! bekoedit: a source-preserving Markdown editor.
//!
//! Desktop entry point: Dioxus Desktop over the OS-native WebView
//! (RFC-002 runtime architecture).

mod app;
mod components;
mod i18n;
mod state;

fn main() {
    dioxus::launch(app::App);
}
