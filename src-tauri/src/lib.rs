mod commands;
mod core;
mod db;
mod plugins;

use std::sync::{Mutex, RwLock};

/// State toàn cục chia sẻ giữa các command
pub struct AppState {
    pub apps: RwLock<Vec<crate::core::indexer::AppEntry>>,
    pub files: RwLock<Vec<crate::core::indexer::FileEntry>>,
    pub db: Mutex<rusqlite::Connection>,
}

pub fn run() {
    let conn = db::open_conn().expect("không mở được database SQLite");
    let state = AppState {
        apps: RwLock::new(Vec::new()),
        files: RwLock::new(Vec::new()),
        db: Mutex::new(conn),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(crate::core::hotkey::build_plugin())
        .manage(state)
        .setup(|app| {
            crate::core::window::setup_window(app)?;
            crate::core::window::setup_tray(app)?;
            crate::core::hotkey::register_shortcuts(app.handle())?;
            crate::core::hotkey::install_winv_hook(app.handle().clone());
            crate::core::indexer::spawn_index_workers(app.handle().clone());
            commands::search::init_everything(app.handle());
            commands::clipboard::spawn_watcher(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                // Cửa sổ chính mất focus -> tự ẩn (hành vi giống Spotlight)
                tauri::WindowEvent::Focused(false) if window.label() == "main" => {
                    use tauri::Emitter;
                    let _ = window.hide();
                    let _ = window.emit("winspot://hidden", ());
                    crate::core::window::trim_memory();
                }
                // Đóng cửa sổ Settings -> chỉ ẩn để mở lại được (không thoát app)
                tauri::WindowEvent::CloseRequested { api, .. } if window.label() == "settings" => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::search::search_all,
            commands::search::fulltext_search,
            commands::capacities::capacities_search,
            commands::capacities::set_capacities_token,
            commands::capacities::capacities_has_token,
            commands::knowledge::wikipedia_search,
            commands::knowledge::wiki_titles,
            commands::knowledge::translate_lookup,
            commands::knowledge::quick_translate,
            commands::knowledge::quick_answer,
            commands::ocr::capture_ocr,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::study::save_study_word,
            commands::study::get_study_words,
            commands::system::system_command,
            commands::system::run_in_terminal,
            commands::system::open_path,
            commands::system::open_url,
            commands::system::copy_text,
            commands::system::run_as_admin,
            commands::system::open_file_location,
            commands::system::uninstall_app,
            commands::system::service_action,
            plugins::window_walker::list_windows,
            plugins::window_walker::focus_window,
            plugins::services::list_services,
            plugins::vscode::vscode_recent,
            plugins::vscode::open_vscode,
            plugins::registry::registry_search,
            plugins::registry::open_regedit,
            plugins::generator::hash_text,
            plugins::processes::list_processes,
            plugins::processes::list_port,
            plugins::processes::kill_process,
            commands::system::get_clipboard_text,
            plugins::passwords::list_passwords,
            plugins::passwords::copy_secret,
            plugins::workflows::list_workflows,
            plugins::workflows::rescan_workflows,
            plugins::workflows::install_workflow,
            plugins::workflows::run_workflow,
            commands::clipboard::get_clipboard_history,
            commands::clipboard::copy_clipboard_item,
            commands::clipboard::paste_clipboard_item,
            commands::clipboard::paste_text,
            commands::clipboard::toggle_pin,
            commands::clipboard::get_clip_image,
            commands::clipboard::update_clipboard_item,
            commands::clipboard::delete_clipboard_item,
            commands::clipboard::clear_clipboard_history,
            commands::clipboard::clear_cache,
            commands::snippets::get_snippets,
            commands::snippets::add_snippet,
            commands::snippets::delete_snippet,
            crate::core::hotkey::suspend_hotkeys,
            crate::core::hotkey::resume_hotkeys,
            crate::core::window::resize_window,
            crate::core::window::hide_and_trim,
            crate::core::window::open_settings_window,
        ])
        .run(tauri::generate_context!())
        .expect("lỗi khi khởi chạy WinSpot");
}
