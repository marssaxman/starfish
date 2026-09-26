#![forbid(unsafe_code)]

mod evaluate;
mod program;
mod render;
mod studio;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([900.0, 660.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Starfish Studio",
        options,
        Box::new(|cc| Ok(Box::new(studio::Studio::new(cc)))),
    )
}
