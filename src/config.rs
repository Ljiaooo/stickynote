use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
pub struct Config {
    pub background: ColorConfig,
    pub text: TextConfig,
    pub padding: PaddingConfig,
    pub corner_radius: f32,
    pub note_file: String,
    pub window_x: Option<f32>,
    pub window_y: Option<f32>,
}

#[derive(Deserialize)]
pub struct ColorConfig {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Deserialize)]
pub struct TextConfig {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
    pub font_family: String,
    pub font_size: f32,
}

#[derive(Deserialize)]
pub struct PaddingConfig {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn bg_color(&self) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(
            self.background.r,
            self.background.g,
            self.background.b,
            self.background.a,
        )
    }

    pub fn text_color(&self) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(
            self.text.r,
            self.text.g,
            self.text.b,
            self.text.a,
        )
    }
}
