#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod bundle;
mod bundle_dialogs;
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
mod objmenu;
mod sample;
mod session;
mod sqltext;
mod store;
#[cfg(test)]
mod memtrack;
#[cfg(test)]
#[global_allocator]
static ALLOC: memtrack::Counting = memtrack::Counting;
mod tabs;

/// Application id of the Linux packages (.desktop, AppStream, Flatpak).
pub const APP_ID: &str = "io.github.moonlipe.Sqlion";

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let shot = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([std::env::var("DBR_W").ok().and_then(|v| v.parse().ok()).unwrap_or(1280.0), 800.0]).with_min_inner_size([800.0, 500.0])
            .with_title("Sqlion")
            // same id as the .desktop file, so Linux desktops group the window with its launcher and icon
            .with_app_id(APP_ID)
            .with_icon(std::sync::Arc::new(eframe::icon_data::from_png_bytes(include_bytes!("../packaging/linux/icons/256.png")).unwrap_or_default())),
        ..Default::default()
    };
    eframe::run_native("Sqlion", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, shot.clone())))))
}
