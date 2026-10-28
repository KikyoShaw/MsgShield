//! MsgShield（消息护盾）库入口

mod apps;
mod commands;
mod editor;
mod error;
mod matcher;
mod modifier;
mod patch_data;
mod path_util;

use commands::PatcherState;
use std::sync::{Arc, RwLock};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(PatcherState {
            bag: Arc::new(RwLock::new(commands::load_bundled_bag())),
            remote_status: Arc::new(RwLock::new(commands::RemoteStatus::Pending)),
        })
        .invoke_handler(tauri::generate_handler![
            commands::detect_apps,
            commands::get_app_status,
            commands::apply_patch,
            commands::restore_backup,
            commands::get_supported_versions,
            commands::get_patch_data_info,
            commands::refresh_patch_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}