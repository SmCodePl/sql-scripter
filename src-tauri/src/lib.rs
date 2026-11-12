mod commands;
mod keychain;
mod state;

use scripter_core::i18n::{self, Lang};
use scripter_core::store::ConnectionStore;
use tauri::Manager;

use crate::state::AppState;

/// File name of the local store inside the app data directory:
/// - macOS:   ~/Library/Application Support/com.sm-code.sqlscripter/connections.db3
/// - Windows: %APPDATA%\com.sm-code.sqlscripter\connections.db3
///
/// Debug builds use a separate file, because they use a different encryption key (see keychain.rs).
#[cfg(not(debug_assertions))]
const STORE_FILE: &str = "connections.db3";
#[cfg(debug_assertions)]
const STORE_FILE: &str = "connections-dev.db3";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            let db_path = app_data.join(STORE_FILE);
            // Key is loaded on first use, so a denied keychain prompt never blocks startup.
            let cipher = keychain::key_source(&app_data);

            // Checks whether the .db3 exists and creates it (with schema) if it does not;
            // migrates a legacy plaintext store to encrypted passwords.
            let opened = tauri::async_runtime::block_on(ConnectionStore::open(&db_path, cipher))?;

            let language = tauri::async_runtime::block_on(opened.store.setting(commands::SETTING_LANGUAGE))?;
            if let Some(lang) = language.as_deref().and_then(Lang::from_code) {
                i18n::set_lang(lang);
            }

            app.manage(AppState::new(opened.store, db_path, opened.created, opened.migrated_connections));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::set_language,
            commands::list_connections,
            commands::create_connection,
            commands::update_connection,
            commands::delete_connection,
            commands::preview_connection_string,
            commands::test_connection,
            commands::open_session,
            commands::open_session_with_password,
            commands::close_session,
            commands::search_objects,
            commands::preview_script,
            commands::export_objects,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SQL Scripter");
}
