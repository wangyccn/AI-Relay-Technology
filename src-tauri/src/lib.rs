// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Set up panic hook to log panics before they crash the app
    std::panic::set_hook(Box::new(|panic_info| {
        let payload = panic_info.payload();
        let message = if let Some(s) = payload.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };

        let location = if let Some(location) = panic_info.location() {
            format!("{}:{}:{}", location.file(), location.line(), location.column())
        } else {
            "Unknown location".to_string()
        };

        // Log the panic with full details
        crate::logger::error(
            "panic",
            &format!(
                "PANIC occurred: message='{}', location='{}', backtrace available via RUST_BACKTRACE=1",
                message, location
            ),
        );

        // Also print to stderr for immediate visibility
        eprintln!("FATAL PANIC: {} at {}", message, location);
    }));

    crate::db::init();
    crate::logger::init();
    crate::logger::info("app", "Application started");
    crate::server::spawn();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
mod adapters;
mod autoconfig;
mod config;
mod db;
mod error;
mod forward;
pub mod logger;
mod pricing;
mod projects;
mod routing;
pub mod server;
mod tools;
