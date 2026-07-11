use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub search_hotkey: String,
    pub clipboard_hotkey: String,
    pub max_clipboard_items: u32,
    pub clipboard_retention_days: u32,
    pub privacy_apps: String,
    pub launch_at_startup: bool,
}

fn value(conn: &rusqlite::Connection, key: &str, default: &str) -> String {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
        .unwrap_or_else(|_| default.to_string())
}

pub fn load(conn: &rusqlite::Connection) -> Settings {
    Settings {
        search_hotkey: value(conn, "search_hotkey", "Alt+Space"),
        clipboard_hotkey: value(conn, "clipboard_hotkey", "Win+V"),
        max_clipboard_items: value(conn, "max_clipboard_items", "500").parse().unwrap_or(500).clamp(50, 2000),
        clipboard_retention_days: value(conn, "clipboard_retention_days", "0").parse().unwrap_or(0).min(3650),
        privacy_apps: value(conn, "privacy_apps", "keepass,bitwarden,1password,lastpass,dashlane,protonpass"),
        launch_at_startup: value(conn, "launch_at_startup", "false") == "true",
    }
}

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, crate::AppState>) -> Result<Settings, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(load(&conn))
}

#[tauri::command]
pub fn save_settings(settings: Settings, app: tauri::AppHandle) -> Result<(), String> {
    if !(50..=2000).contains(&settings.max_clipboard_items) { return Err("Giới hạn clipboard phải từ 50 đến 2000".into()); }
    if settings.clipboard_retention_days > 3650 { return Err("Thời gian lưu clipboard tối đa là 3650 ngày".into()); }
    crate::core::hotkey::apply_hotkeys(&app, &settings.search_hotkey, &settings.clipboard_hotkey)?;
    set_startup(settings.launch_at_startup)?;
    let state = app.state::<crate::AppState>();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    for (key, val) in [
        ("search_hotkey", settings.search_hotkey),
        ("clipboard_hotkey", settings.clipboard_hotkey),
        ("max_clipboard_items", settings.max_clipboard_items.to_string()),
        ("clipboard_retention_days", settings.clipboard_retention_days.to_string()),
        ("privacy_apps", settings.privacy_apps),
        ("launch_at_startup", settings.launch_at_startup.to_string()),
    ] {
        conn.execute("INSERT INTO settings (key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, val])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn set_startup(enabled: bool) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run").map_err(|e| e.to_string())?;
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        key.set_value("WinSpot", &format!("\"{}\"", exe.display())).map_err(|e| e.to_string())
    } else {
        match key.delete_value("WinSpot") { Ok(_) => Ok(()), Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()), Err(e) => Err(e.to_string()) }
    }
}
