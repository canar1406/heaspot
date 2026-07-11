//! VS Code Workspaces (`{`): đọc lịch sử project từ state.vscdb (SQLite)
//! tại %APPDATA%\Code\User\globalStorage, mở thẳng bằng VS Code.

use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
pub struct VsCodeEntry {
    pub name: String,
    pub path: String,
}

fn state_db_path() -> Option<PathBuf> {
    let p = dirs::config_dir()?.join(r"Code\User\globalStorage\state.vscdb");
    p.exists().then_some(p)
}

/// file:///c%3A/Users/x → C:\Users\x
fn uri_to_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file:///")?;
    let decoded = percent_decode(rest);
    Some(decoded.replace('/', "\\"))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

#[tauri::command]
pub fn vscode_recent(query: String) -> Vec<VsCodeEntry> {
    let Some(db) = state_db_path() else {
        return Vec::new();
    };
    // DB có thể đang bị VS Code giữ -> copy ra temp rồi đọc
    let value: Option<String> = read_history_json(&db).or_else(|| {
        let tmp = std::env::temp_dir().join("winspot_vscdb_copy.db");
        std::fs::copy(&db, &tmp).ok()?;
        read_history_json(&tmp)
    });
    let Some(json) = value else {
        return Vec::new();
    };

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
    let entries = parsed
        .get("entries")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    let q = query.trim().to_lowercase();
    let mut out = Vec::new();
    for e in entries {
        let uri = e
            .get("folderUri")
            .or_else(|| e.get("fileUri"))
            .or_else(|| e.get("workspace").and_then(|w| w.get("configPath")))
            .and_then(|u| u.as_str())
            .unwrap_or("");
        let Some(path) = uri_to_path(uri) else {
            continue;
        };
        let name = std::path::Path::new(&path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        if q.is_empty() || name.to_lowercase().contains(&q) || path.to_lowercase().contains(&q) {
            out.push(VsCodeEntry { name, path });
        }
        if out.len() >= 12 {
            break;
        }
    }
    out
}

fn read_history_json(db: &PathBuf) -> Option<String> {
    let conn = rusqlite::Connection::open_with_flags(
        db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .ok()?;
    conn.query_row(
        "SELECT value FROM ItemTable WHERE key = 'history.recentlyOpenedPathsList'",
        [],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

/// Mở project bằng VS Code (ưu tiên `code` trong PATH, fallback Code.exe)
#[tauri::command]
pub fn open_vscode(path: String) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let via_cli = std::process::Command::new("cmd")
        .args(["/C", "code", &path])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if via_cli.is_ok() {
        return Ok(());
    }
    let exe = dirs::data_local_dir()
        .map(|d| d.join(r"Programs\Microsoft VS Code\Code.exe"))
        .filter(|p| p.exists())
        .ok_or("không tìm thấy VS Code")?;
    std::process::Command::new(exe)
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
