use rusqlite::Connection;
use std::path::PathBuf;

/// Đường dẫn DB: %APPDATA%\heaspot\heaspot.db
pub fn db_path() -> PathBuf {
    // Cho smoke test/documentation chạy bằng profile tạm, tuyệt đối không đọc
    // clipboard/settings thật. Bản phát hành bình thường không đặt biến này.
    if let Ok(dir) = std::env::var("HEASPOT_DATA_DIR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            return PathBuf::from(dir).join("heaspot.db");
        }
    }
    let base = dirs::data_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("heaspot").join("heaspot.db")
}

/// Mở kết nối SQLite (mỗi thread một connection, WAL cho phép đọc/ghi song song)
pub fn open_conn() -> rusqlite::Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(&path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS clipboard (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'text',
            created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
        );
        CREATE TABLE IF NOT EXISTS snippets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            keyword TEXT NOT NULL UNIQUE,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
        );
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS study_words (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            word TEXT NOT NULL UNIQUE,
            translation TEXT NOT NULL,
            details TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
        );",
    )?;
    // Migration cho bảng clipboard cũ (bỏ qua lỗi nếu cột đã tồn tại)
    let _ = conn.execute(
        "ALTER TABLE clipboard ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE clipboard ADD COLUMN source_app TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE clipboard ADD COLUMN thumb TEXT NOT NULL DEFAULT ''",
        [],
    );
    Ok(conn)
}

/// Thư mục cache ảnh clipboard: %APPDATA%\winspot\clips
pub fn clips_dir() -> PathBuf {
    let dir = db_path()
        .parent()
        .map(|p| p.join("clips"))
        .unwrap_or_else(|| PathBuf::from("clips"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}
