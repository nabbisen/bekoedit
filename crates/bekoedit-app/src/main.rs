//! bekoedit: a source-preserving Markdown editor.

mod app;
mod bridge;
mod components;
mod error_keys;
mod i18n;
mod settings;
mod smoke_test;
pub mod source_sync;
mod state;

fn dioxus_config() -> dioxus::desktop::Config {
    dioxus::desktop::Config::new()
        .with_window(
            dioxus::desktop::WindowBuilder::new()
                .with_title("bekoedit")
                .with_inner_size(dioxus::desktop::LogicalSize::new(1200.0, 800.0)),
        )
        // Suppress the default OS native menu (prevents the broken Window menu).
        .with_menu(None)
}

fn main() {
    if std::env::args().any(|a| a == "--headless-smoke") {
        smoke_test::run();
        std::process::exit(0);
    }
    dioxus::LaunchBuilder::desktop()
        .with_cfg(dioxus_config())
        .launch(app::App);
}

#[cfg(test)]
mod tests;
