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
        );
        CREATE TABLE IF NOT EXISTS launch_usage (
            path TEXT PRIMARY KEY,
            launch_count INTEGER NOT NULL DEFAULT 0,
            last_launched TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
        );
        CREATE TABLE IF NOT EXISTS otp_accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            note TEXT NOT NULL DEFAULT '',
            issuer TEXT NOT NULL DEFAULT '',
            secret_enc BLOB NOT NULL,
            otp_type TEXT NOT NULL DEFAULT 'totp',
            algorithm TEXT NOT NULL DEFAULT 'SHA1',
            digits INTEGER NOT NULL DEFAULT 6,
            period INTEGER NOT NULL DEFAULT 30,
            counter INTEGER NOT NULL DEFAULT 0,
            is_pinned INTEGER NOT NULL DEFAULT 0,
            is_archived INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
        );
        CREATE TABLE IF NOT EXISTS otp_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id INTEGER NOT NULL,
            account_name TEXT NOT NULL,
            code_enc BLOB NOT NULL,
            generated_at INTEGER NOT NULL,
            valid_until INTEGER NOT NULL,
            copied INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY(account_id) REFERENCES otp_accounts(id) ON DELETE CASCADE
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
    let _ = conn.execute(
        "ALTER TABLE clipboard ADD COLUMN content_hash TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_clipboard_content_hash ON clipboard(kind, content_hash)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_otp_accounts_state ON otp_accounts(is_archived, is_pinned DESC, name)",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE otp_accounts ADD COLUMN note TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_otp_history_time ON otp_history(generated_at DESC)",
        [],
    );
    // One-time cleanup for old text/link/file duplicates. Keep the newest row
    // and preserve pin state on it before removing the stale copies.
    let _ = conn.execute_batch(
        "UPDATE clipboard SET is_pinned = 1
           WHERE id IN (
             SELECT MAX(id) FROM clipboard
             WHERE kind IN ('text','link','files')
             GROUP BY kind, content
             HAVING MAX(is_pinned) = 1
           );
         DELETE FROM clipboard
           WHERE kind IN ('text','link','files')
             AND id NOT IN (
               SELECT MAX(id) FROM clipboard
               WHERE kind IN ('text','link','files')
               GROUP BY kind, content
             );",
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
