mod commands;
mod core;
mod db;
mod plugins;

use std::sync::{Mutex, RwLock};
use tauri::Manager;

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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(crate::core::hotkey::build_plugin())
        .manage(state)
        .setup(|app| {
            crate::core::window::setup_window(app)?;
            crate::core::window::setup_tray(app)?;
            crate::core::hotkey::register_shortcuts(app.handle())?;
            crate::core::hotkey::install_winv_hook(app.handle().clone());
            crate::core::window::spawn_focus_watchdog(app.handle().clone());
            commands::updater::spawn_auto_check(app.handle().clone());
            crate::core::indexer::spawn_index_workers(app.handle().clone());
            commands::search::init_everything(app.handle());
            commands::clipboard::spawn_watcher(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::Focused(true) if window.label() == "main" => {
                    crate::core::window::mark_main_focused();
                }
                // Cửa sổ chính mất focus -> tự ẩn (hành vi giống Spotlight)
                tauri::WindowEvent::Focused(false) if window.label() == "main" => {
                    // Delay very briefly: native dialogs owned by HeaSpot also
                    // cause a blur event, but must not close the launcher. The
                    // foreground process check distinguishes them from another
                    // application receiving focus.
                    let app = window.app_handle().clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        if crate::core::window::should_hide_for_focus_loss() {
                            crate::core::window::hide_main(&app);
                        }
                    });
                }
                // Đóng cửa sổ Settings -> chỉ ẩn để mở lại được (không thoát app)
                tauri::WindowEvent::CloseRequested { api, .. } if window.label() == "settings" => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                // The progress window may outlive the launcher for a long
                // uninstall. Closing while active minimizes it instead of
                // losing the only observer; the tray can restore it.
                tauri::WindowEvent::CloseRequested { api, .. }
                    if window.label() == "uninstall-progress" =>
                {
                    api.prevent_close();
                    if crate::commands::system::uninstall_is_running() {
                        let _ = window.minimize();
                    } else {
                        let _ = window.hide();
                    }
                }
                tauri::WindowEvent::CloseRequested { api, .. }
                    if window.label() == "update-progress" =>
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::search::search_all,
            commands::search::load_result_icons,
            commands::search::fulltext_search,
            commands::capacities::capacities_search,
            commands::capacities::set_capacities_token,
            commands::capacities::capacities_has_token,
            commands::knowledge::wikipedia_search,
            commands::knowledge::wiki_titles,
            commands::knowledge::translate_lookup,
            commands::knowledge::quick_translate,
            commands::knowledge::quick_answer,
            commands::currency::currency_convert,
            commands::ocr::capture_ocr,
            commands::otp::preview_otp,
            commands::otp::add_otp_account,
            commands::otp::quick_add_otp_account,
            commands::otp::list_otp_accounts,
            commands::otp::rename_otp_account,
            commands::otp::toggle_otp_pin,
            commands::otp::set_otp_archived,
            commands::otp::delete_otp_account,
            commands::otp::copy_otp_code,
            commands::otp::get_otp_secret,
            commands::otp::copy_otp_secret,
            commands::otp::list_otp_history,
            commands::otp::clear_otp_history,
            commands::otp::export_otp_backup,
            commands::otp::import_otp_backup,
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
            commands::system::get_uninstall_progress,
            commands::updater::check_for_updates,
            commands::updater::install_available_update,
            commands::updater::get_update_progress,
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
            crate::core::window::show_and_focus_main,
            crate::core::window::restore_main_window,
            crate::core::window::hide_and_trim,
            crate::core::window::open_settings_window,
        ])
        .build(tauri::generate_context!())
        .expect("lỗi khi khởi tạo HeaSpot")
        .run(|_, event| {
            if let tauri::RunEvent::Exit = event {
                crate::commands::search::shutdown_everything();
            }
        });
}
