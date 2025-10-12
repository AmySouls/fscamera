#![windows_subsystem = "windows"]

use app::CameraControlApp;
use eframe::egui::ViewportBuilder;

mod controls;
mod process;
mod save;
mod app;
mod settings;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size((800.0, 400.0)),
        ..Default::default()
    };

    eframe::run_native(
        &format!("Freecum+ v{}", env!("CARGO_PKG_VERSION")),
        options,
        Box::new(|_cc| Ok(Box::new(CameraControlApp::default()))),
    )
}

pub(crate) fn program_title() -> String {
    format!("Freecum+ v{}", env!("CARGO_PKG_VERSION"))
}
