#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::{get_entropy, extract_file, deep_scan, open_file_location};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            get_entropy,
            extract_file,
            deep_scan,
            open_file_location
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
