mod app;
mod icons;
mod theme;
mod constants;
mod shared;
mod dialogs;
mod db;
mod editing;
mod drivers;
mod export;
mod git;
mod grid;
mod sample;
mod sqltext;
mod store;
mod tabs;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let shot = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([std::env::var("DBR_W").ok().and_then(|v| v.parse().ok()).unwrap_or(1280.0), 800.0]).with_min_inner_size([800.0, 500.0]).with_title("Sqlion"),
        ..Default::default()
    };
    eframe::run_native("Sqlion", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, shot.clone())))))
}
