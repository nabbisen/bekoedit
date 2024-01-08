
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use std::ops::Deref;
use std::fs;

use uuid::Uuid;
use serde::Serialize;

use tauri::{Manager, State, Size, PhysicalSize, Position, PhysicalPosition};

use pulldown_cmark::{html, Options, Parser};
use mdka::from_html;

mod config;

#[derive(Default, Serialize)]
struct AppState {
    user_settings: Mutex<config::UserSettings>,
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn md2html(markdown: &str) -> String {
    let mut options = Options::empty();
    for option in vec![
        Options::ENABLE_TABLES,
        Options::ENABLE_HEADING_ATTRIBUTES,
        Options::ENABLE_STRIKETHROUGH,
        Options::ENABLE_TASKLISTS,
        Options::ENABLE_FOOTNOTES,
    ] {
        options.insert(option);
    }
    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    
    html2contenteditable(html_output.as_str())
}

fn html2contenteditable(html: &str) -> String {
    let headings: Vec<String> = (1..=6).map(|x| format!("h{}", x)).collect();
    // "pre", "code", "blockquote" are not editable (use md editor instead)
    let node_types: Vec<String> = vec!["li", "p", "div", "span", "td", "th", "tbody", "thead"].iter().map(|x| format!("{}", x)).collect();
    ([headings, node_types].concat()).iter().fold(html.to_string(), |acc: String, from| {
        acc.replace(format!("<{}>", from).as_str(), format!("<{} contenteditable=\"true\">", from).as_str())
    })
}

#[tauri::command]
fn html2md(html: &str) -> String {
    from_html(&html)
}

#[tauri::command]
fn uuid() -> String {
    Uuid::new_v4().simple().to_string()
}

#[tauri::command]
fn user_settings(state: State<'_, AppState>) -> String {
    let user_settings = state.user_settings.lock().unwrap();
    serde_json::to_string(user_settings.deref()).unwrap()
}

#[tauri::command]
fn update_window(state: State<'_, AppState>, width: u32, height: u32, x: i32, y: i32) {
    let app_state = state.clone();
    app_state.user_settings.lock().unwrap().update_window(width, height, x, y);
}

#[tauri::command]
fn update_md_editor_width(state: State<'_, AppState>, md_editor_width: u8) {
    let app_state = state.clone();
    app_state.user_settings.lock().unwrap().update_md_editor_width(md_editor_width);
}

#[tauri::command]
fn update_is_light(state: State<'_, AppState>, is_light: bool) {
    let app_state = state.clone();
    if is_light {
        app_state.user_settings.lock().unwrap().update_color_light()
    } else {
        app_state.user_settings.lock().unwrap().update_color_dark()
    }
}

#[tauri::command]
fn update_font_family(state: State<'_, AppState>, font_family: &str) {
    let app_state = state.clone();
    app_state.user_settings.lock().unwrap().update_font_family(font_family);
}

#[tauri::command]
fn update_font_size(state: State<'_, AppState>, font_size: u8) {
    let app_state = state.clone();
    app_state.user_settings.lock().unwrap().update_font_size(font_size);
}

#[tauri::command]
fn read_textfile(state: State<'_, AppState>, filepath: &str, is_html: bool) -> String {
    let read = fs::read_to_string(filepath);

    match read {
        Ok(contents) => {
            let app_state = state.clone();
            app_state.user_settings.lock().unwrap().update_startup_filepath(filepath);
    
            if is_html {
                html2contenteditable(&contents)
            } else {
                contents
            }
        },
        _ => String::new()
    }    
}

fn main() {
    // quesition: let mut user_settings = config::UserSettings::default();
    tauri::Builder::default()
        .setup(move |app| {
            let window = app.get_window("main").unwrap();
            // devtools start
            #[cfg(debug_assertions)] // only include this code on debug builds
            {
              window.open_devtools();
            //   window.close_devtools();
            }
            
            let app_state = AppState::default();
            // question: config::DIRNAME.set(app.config().package.product_name.clone().unwrap());
            app_state.user_settings.lock().unwrap().init(&app.config().package.product_name.clone().unwrap());

            let mut user_settings = app_state.user_settings.lock().unwrap().clone();
            app.manage(app_state);
            window.set_size(Size::Physical(PhysicalSize { width: user_settings.window_size_width(), height: user_settings.window_size_height() })).unwrap();
            window.set_position(Position::Physical(PhysicalPosition { x: user_settings.window_position_x(), y: user_settings.window_position_y() })).unwrap();
            Ok(())
        })
        // devtools end
        .invoke_handler(tauri::generate_handler![md2html, html2md, uuid, user_settings, update_window, update_md_editor_width, update_is_light, update_font_family, update_font_size, read_textfile])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

