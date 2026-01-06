#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_iap::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
