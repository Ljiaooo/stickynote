#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod watcher;

use std::path::Path;

fn main() {
    let base_dir = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());

    let config_path = base_dir.join("config.toml");
    let config = config::Config::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config: {e}");
        std::process::exit(1);
    });

    let note_path = base_dir.join(&config.note_file);
    let text = std::fs::read_to_string(&note_path).unwrap_or_default();

    let (tx, rx) = std::sync::mpsc::channel();
    let file_watcher = watcher::start_watcher(&note_path, tx).unwrap_or_else(|e| {
        eprintln!("Failed to start file watcher: {e}");
        std::process::exit(1);
    });

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("StickyNote")
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_inner_size(egui::vec2(300.0, 100.0));

    if let (Some(x), Some(y)) = (config.window_x, config.window_y) {
        viewport = viewport.with_position(egui::pos2(x, y));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let font_path = config.text.font_family.clone();

    eframe::run_native(
        "StickyNote",
        options,
        Box::new(move |cc| {
            if let Ok(font_data) = std::fs::read(&font_path) {
                let mut fonts = egui::FontDefinitions::default();
                fonts.font_data.insert(
                    "custom".to_owned(),
                    std::sync::Arc::new(egui::FontData::from_owned(font_data)),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "custom".to_owned());
                cc.egui_ctx.set_fonts(fonts);
            }

            Ok(Box::new(app::StickyNoteApp::new(
                config,
                note_path,
                text,
                rx,
                file_watcher,
            )))
        }),
    )
    .unwrap();
}
