#![windows_subsystem = "windows"] // pas de fenêtre console

mod app;
mod model;
mod theme;
mod update;
mod win;

use eframe::egui;
use std::sync::Arc;

fn load_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../assets/launcher.ico")).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width, height })
}

fn main() -> eframe::Result<()> {
    // Relancé après une mise à jour : on attend que l'ancienne instance soit fermée
    // (contrairement à Ronnie sous Windows, qui relance sans attendre).
    let args: Vec<String> = std::env::args().collect();
    if let Some(pid) = args.iter().position(|a| a == "--after-update").and_then(|i| args.get(i + 1)) {
        if let Ok(pid) = pid.parse() {
            win::wait_for_process(pid, 10_000);
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Launcher")
        .with_inner_size([1240.0, 760.0])
        .with_min_inner_size([900.0, 560.0])
        .with_drag_and_drop(true);
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    eframe::run_native("Launcher", options, Box::new(|cc| Ok(Box::new(app::LauncherApp::new(cc)))))
}
