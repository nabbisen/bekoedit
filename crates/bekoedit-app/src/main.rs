//! bekoedit: a source-preserving Markdown editor.

mod app;
mod components;
mod i18n;
mod settings;
mod state;

fn main() {
    dioxus::launch(app::App);
}
