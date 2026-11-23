#![windows_subsystem = "windows"] // pas de fenêtre console

mod app;
mod dragon;
mod model;
mod theme;
mod updater;
mod win;

use eframe::egui;
use std::sync::Arc;

fn load_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../assets/winton.ico")).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width, height })
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Winton")
        .with_inner_size([1240.0, 760.0])
        .with_min_inner_size([900.0, 560.0])
        .with_drag_and_drop(true);
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    eframe::run_native("Winton", options, Box::new(|cc| Ok(Box::new(app::LauncherApp::new(cc)))))
}
