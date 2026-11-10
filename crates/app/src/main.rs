//! Punto de entrada: ventana de escritorio (eframe/egui).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // sin consola en Windows release

use eframe::egui;

/// Icono de la ventana y de la barra de tareas, embebido en el binario.
fn window_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../../../assets/icono-parte-diario.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = (img.width(), img.height());
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result<()> {
    let _log_guard = parte_diario_config::logging::init_logging();
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1400.0, 880.0])
        .with_min_inner_size([1150.0, 680.0])
        .with_title("Parte Diario — Anti-Ruidos y Contaminación Sónica, P.N.");
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "parte-diario",
        options,
        Box::new(|cc| Ok(Box::new(parte_diario_app::gui::App::new(cc)))),
    )
}
