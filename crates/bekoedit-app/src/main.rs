//! bekoedit: a source-preserving Markdown editor.

mod app;
mod bridge;
mod components;
mod i18n;
mod settings;
mod smoke_test;
mod state;

fn main() {
    if std::env::args().any(|a| a == "--headless-smoke") {
        smoke_test::run();
        std::process::exit(0);
    }
    dioxus::launch(app::App);
}
