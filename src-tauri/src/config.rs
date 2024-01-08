use std::fs;
use std::fs::File;
use std::path::Path;
use std::io::Write;
use serde::{Serialize, Deserialize};
use toml;

use tauri::api::path::config_dir;

const USER_SETTINGS_FILENAME: &str = "user-settings.toml";

#[warn(dead_code)]
#[derive(Serialize, Deserialize, Clone)]
pub enum Color {
    Light,
    Dark,
}
#[warn(dead_code)]
#[derive(Serialize, Deserialize, Clone)]
pub enum FontFamily {
    Monospace1,
    SansSerif1,
    SansSerif2,
    Serif1,
}
impl FontFamily {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "Monospace1" => Some(Self::Monospace1),
            "SansSerif1" => Some(Self::SansSerif1),
            "SansSerif2" => Some(Self::SansSerif2),
            "Serif1" => Some(Self::Serif1),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UserSettings {
    window_size_width: u32,
    window_size_height: u32,
    window_position_x: i32,
    window_position_y: i32,
    md_editor_width: u8,
    color: Color,
    font_family: FontFamily,
    font_size: u8,
    startup_filepath: Option<String>,
    #[serde(skip_serializing)]
    selfpath: Option<String>,
}
impl Default for UserSettings {
    fn default() -> Self {
        UserSettings {
            window_size_width: 800,
            window_size_height: 600,
            window_position_x: 0,
            window_position_y: 0,
            md_editor_width: 50,
            color: Color::Dark,
            font_family: FontFamily::Monospace1,
            font_size: 15,
            startup_filepath: None,
            selfpath: None,
        }
    }
}
impl UserSettings {
    pub fn init(&mut self, product_name: &str) {
        let dirname = product_name;
        self.selfpath = Some(generate_selfpath(USER_SETTINGS_FILENAME, dirname));
        validate_dir(dirname);
        let mut init_value: UserSettings;
        match fs::read_to_string(self.selfpath.clone().unwrap().to_owned()) {
            Ok(content) => init_value = toml::from_str(&content).unwrap_or_default(),
            // todo: init value
            _ => { self.font_size = 16; return; },
        }
        init_value.selfpath = Some(self.selfpath.clone().unwrap().to_owned());
        *self = init_value;
    }
    pub fn write(&self) -> std::io::Result<()> {
        let mut file = File::create(&self.selfpath.clone().unwrap()).unwrap();
        let content = toml::to_string(&self).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        Ok(())
    }
    pub fn window_size_width(&mut self) -> u32 {
        self.window_size_width
    }
    pub fn window_size_height(&mut self) -> u32 {
        self.window_size_height
    }
    pub fn window_position_x(&mut self) -> i32 {
        self.window_position_x
    }
    pub fn window_position_y(&mut self) -> i32 {
        self.window_position_y
    }
    pub fn update_window(&mut self, width: u32, height: u32, x: i32, y: i32) {
        self.window_size_width = width;
        self.window_size_height = height;
        self.window_position_x = x;
        self.window_position_y = y;
        self.write().unwrap();
    }
    pub fn update_md_editor_width(&mut self, md_editor_width: u8) {
        self.md_editor_width = md_editor_width;
    }
    pub fn update_color_dark(&mut self) {
        self.color = Color::Dark;
        self.write().unwrap();
    }
    pub fn update_color_light(&mut self) {
        self.color = Color::Light;
        self.write().unwrap();
    }
    pub fn update_font_family(&mut self, font_family: &str) {
        self.font_family = FontFamily::from_str(font_family).unwrap();
        self.write().unwrap();
    }
    pub fn update_font_size(&mut self, font_size: u8) {
        self.font_size = font_size;
        self.write().unwrap();
    }
    pub fn update_startup_filepath(&mut self, startup_filepath: &str) {
        self.startup_filepath = Some(startup_filepath.to_string());
        self.write().unwrap();
    }
}

pub fn validate_dir(dirname: &str) -> bool {
    let dirpath = generate_dirpath(dirname);
    let path = Path::new(&dirpath);
    path.exists() || fs::create_dir_all(path).is_ok()
}

fn generate_selfpath(filename: &str, dirname: &str) -> String {
    format!("{}/{}", generate_dirpath(dirname), filename)
}

fn generate_dirpath(dirname: &str) -> String {
    format!("{}/{}", config_dir().unwrap().into_os_string().into_string().unwrap(), dirname)
}
