use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    // Hotkey toàn cục để MỞ app
    pub search_hotkey: String,
    pub clipboard_hotkey: String,
    // Keyword/cú pháp kích hoạt từng tính năng — JSON { featureId: keyword }.
    // Backend chỉ lưu; router ở frontend đọc và định tuyến theo giá trị này.
    pub keywords: String,
    // JSON { featureId: "Alt+Shift+T" } — hotkey toàn cục mở thẳng feature.
    pub feature_hotkeys: String,
    // Clipboard & Privacy
    pub max_clipboard_items: u32,
    pub clipboard_retention_days: u32,
    pub privacy_apps: String,
    pub auto_paste: bool,
    // Serper.dev (Google SERP) key (tùy chọn) — có key thì `g` phủ mọi truy vấn web
    pub serper_api_key: String,
    // Tính năng nhạy cảm — mặc định tắt, người dùng tự bật
    pub enable_browser_passwords: bool,
    // Có lưu mật khẩu vào clipboard history khi copy không (mặc định KHÔNG)
    pub password_to_history: bool,
    // Chung
    pub theme: String, // "system" | "light" | "dark"
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
        keywords: value(conn, "keywords", "{}"),
        feature_hotkeys: value(conn, "feature_hotkeys", "{}"),
        max_clipboard_items: value(conn, "max_clipboard_items", "500").parse().unwrap_or(500).clamp(50, 2000),
        clipboard_retention_days: value(conn, "clipboard_retention_days", "0").parse().unwrap_or(0).min(3650),
        privacy_apps: value(conn, "privacy_apps", "keepass,bitwarden,1password,lastpass,dashlane,protonpass"),
        auto_paste: value(conn, "auto_paste", "true") == "true",
        serper_api_key: value(conn, "serper_api_key", ""),
        enable_browser_passwords: value(conn, "enable_browser_passwords", "false") == "true",
        password_to_history: value(conn, "password_to_history", "false") == "true",
        theme: value(conn, "theme", "system"),
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
    if !(50..=2000).contains(&settings.max_clipboard_items) {
        return Err("Giới hạn clipboard phải từ 50 đến 2000".into());
    }
    if settings.clipboard_retention_days > 3650 {
        return Err("Thời gian lưu clipboard tối đa là 3650 ngày".into());
    }
    crate::core::hotkey::apply_hotkeys(
        &app,
        &settings.search_hotkey,
        &settings.clipboard_hotkey,
        &settings.feature_hotkeys,
        &settings.keywords,
    )?;
    set_startup(settings.launch_at_startup)?;

    let state = app.state::<crate::AppState>();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    for (key, val) in [
        ("search_hotkey", settings.search_hotkey.clone()),
        ("clipboard_hotkey", settings.clipboard_hotkey.clone()),
        ("keywords", settings.keywords.clone()),
        ("feature_hotkeys", settings.feature_hotkeys.clone()),
        ("max_clipboard_items", settings.max_clipboard_items.to_string()),
        ("clipboard_retention_days", settings.clipboard_retention_days.to_string()),
        ("privacy_apps", settings.privacy_apps.clone()),
        ("auto_paste", settings.auto_paste.to_string()),
        ("serper_api_key", settings.serper_api_key.clone()),
        ("enable_browser_passwords", settings.enable_browser_passwords.to_string()),
        ("password_to_history", settings.password_to_history.to_string()),
        ("theme", settings.theme.clone()),
        ("launch_at_startup", settings.launch_at_startup.to_string()),
    ] {
        conn.execute(
            "INSERT INTO settings (key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, val],
        )
        .map_err(|e| e.to_string())?;
    }
    // Báo cửa sổ chính reload settings (theme, keyword) ngay lập tức
    use tauri::Emitter;
    let _ = app.emit("winspot://settings-changed", ());
    Ok(())
}

fn set_startup(enabled: bool) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .map_err(|e| e.to_string())?;
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        key.set_value("HeaSpot", &format!("\"{}\"", exe.display())).map_err(|e| e.to_string())
    } else {
        match key.delete_value("HeaSpot") {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
