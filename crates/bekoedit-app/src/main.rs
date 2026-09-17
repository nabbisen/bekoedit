//! bekoedit: a source-preserving Markdown editor.

// Release builds on Windows link as a GUI program, so launching bekoedit.exe
// from Explorer, the Start menu or a Store install opens no console window
// beside the app, and closing one cannot kill it (task 018). Debug builds keep
// the console for development output. The attribute is ignored on Linux and
// macOS. A release binary started from a terminal prints nothing to it; output
// redirected to a file or pipe still arrives, and exit codes are unchanged.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bridge;
mod components;
mod error_keys;
mod i18n;
mod menu_entry;
mod persistence;
mod settings;
mod shell_focus;
mod smoke_test;
pub mod source_sync;
mod state;
mod webview_smoke;

fn dioxus_config(smoke_run: Option<webview_smoke::SmokeRun>) -> dioxus::desktop::Config {
    let config = dioxus::desktop::Config::new()
        .with_window(
            dioxus::desktop::WindowBuilder::new()
                .with_title("bekoedit")
                .with_inner_size(dioxus::desktop::LogicalSize::new(1200.0, 800.0)),
        )
        // Suppress the default OS native menu (prevents the broken Window menu).
        .with_menu(None);
    let Some(smoke_run) = smoke_run else {
        return config;
    };
    let mut smoke_run = Some(smoke_run);
    config.with_custom_event_handler(move |event, _| {
        if matches!(event, dioxus::desktop::tao::event::Event::LoopDestroyed) {
            let run = smoke_run
                .take()
                .expect("the desktop event loop is destroyed only once");
            std::process::exit(run.finalize_exit_code());
        }
    })
}

fn main() {
    let run_mode =
        webview_smoke::RunMode::parse(std::env::args_os().skip(1)).unwrap_or_else(|error| {
            eprintln!("bekoedit: {error}");
            std::process::exit(2);
        });
    if run_mode == webview_smoke::RunMode::HeadlessSmoke {
        smoke_test::run();
        return;
    }
    let smoke_run = webview_smoke::prepare_launch(run_mode).unwrap_or_else(|error| {
        eprintln!("bekoedit: {error}");
        std::process::exit(2);
    });
    dioxus::LaunchBuilder::desktop()
        .with_cfg(dioxus_config(smoke_run))
        .launch(app::App);
}

#[cfg(test)]
mod tests;
