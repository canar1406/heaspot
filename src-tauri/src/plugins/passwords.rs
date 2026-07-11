//! Trình quản lý mật khẩu trình duyệt (`pw`) — TÙY CHỌN, mặc định TẮT.
//!
//! Đọc mật khẩu đã lưu của Edge / Brave / Chrome / Cốc Cốc… cho CHÍNH tài khoản
//! Windows đang đăng nhập. Cơ chế giống tính năng "Export passwords" của trình duyệt:
//! - `Local State` chứa khóa AES bọc bằng DPAPI (chỉ user hiện tại giải mã được).
//! - `Login Data` (SQLite) chứa mật khẩu mã hóa AES-256-GCM (prefix v10/v11) hoặc DPAPI (cũ).
//!
//! Chỉ chạy khi người dùng bật trong Settings. Mật khẩu chỉ giải mã cục bộ theo yêu cầu,
//! không ghi log, và khi copy sẽ dùng format loại trừ để KHÔNG lọt vào clipboard history.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
pub struct BrowserPassword {
    pub browser: String,
    pub url: String,
    pub username: String,
    pub password: String,
}

struct Browser {
    name: &'static str,
    user_data: PathBuf,
}

fn browsers() -> Vec<Browser> {
    let local = dirs::data_local_dir().unwrap_or_default();
    let roaming = dirs::config_dir().unwrap_or_default();
    let mut list = vec![
        ("Edge", local.join(r"Microsoft\Edge\User Data")),
        ("Chrome", local.join(r"Google\Chrome\User Data")),
        ("Brave", local.join(r"BraveSoftware\Brave-Browser\User Data")),
        ("Cốc Cốc", local.join(r"CocCoc\Browser\User Data")),
        ("Opera", roaming.join(r"Opera Software\Opera Stable")),
        ("Vivaldi", local.join(r"Vivaldi\User Data")),
    ];
    list.retain(|(_, p)| p.exists());
    list.into_iter().map(|(name, user_data)| Browser { name, user_data }).collect()
}

/// DPAPI: giải mã blob về plaintext (CryptUnprotectData, phạm vi user hiện tại)
unsafe fn dpapi_decrypt(data: &[u8]) -> Option<Vec<u8>> {
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};
    let mut in_blob = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut out_blob = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    let ok = CryptUnprotectData(
        &mut in_blob,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        0,
        &mut out_blob,
    );
    if ok == 0 || out_blob.pbData.is_null() {
        return None;
    }
    let out = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
    windows_sys::Win32::Foundation::LocalFree(out_blob.pbData as _);
    Some(out)
}

/// Lấy khóa AES từ Local State (bọc DPAPI)
fn master_key(user_data: &PathBuf) -> Option<Vec<u8>> {
    let local_state = std::fs::read_to_string(user_data.join("Local State")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&local_state).ok()?;
    let b64 = v.get("os_crypt")?.get("encrypted_key")?.as_str()?;
    let raw = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    // Bỏ 5 byte prefix "DPAPI"
    if raw.len() < 5 || &raw[..5] != b"DPAPI" {
        return None;
    }
    unsafe { dpapi_decrypt(&raw[5..]) }
}

/// Giải mã 1 password_value
fn decrypt_value(enc: &[u8], key: &[u8]) -> Option<String> {
    if enc.len() > 3 && (&enc[..3] == b"v10" || &enc[..3] == b"v11") {
        if enc.len() < 3 + 12 + 16 {
            return None;
        }
        let nonce = &enc[3..15];
        let ciphertext = &enc[15..];
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let plain = cipher
            .decrypt(Nonce::from_slice(nonce), Payload { msg: ciphertext, aad: b"" })
            .ok()?;
        return String::from_utf8(plain).ok();
    }
    // Legacy: DPAPI trực tiếp
    unsafe { dpapi_decrypt(enc) }.and_then(|b| String::from_utf8(b).ok())
}

fn read_profile(browser: &str, key: &[u8], login_db: &PathBuf, out: &mut Vec<BrowserPassword>) {
    // Login Data thường bị trình duyệt khóa -> copy ra temp rồi đọc read-only
    let tmp = std::env::temp_dir().join(format!("heaspot_logins_{}.db", std::process::id()));
    if std::fs::copy(login_db, &tmp).is_err() {
        return;
    }
    if let Ok(conn) = rusqlite::Connection::open_with_flags(
        &tmp,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT origin_url, username_value, password_value FROM logins",
        ) {
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                ))
            });
            if let Ok(rows) = rows {
                for row in rows.flatten() {
                    let (url, username, enc) = row;
                    if enc.is_empty() {
                        continue;
                    }
                    if let Some(password) = decrypt_value(&enc, key) {
                        if password.is_empty() {
                            continue;
                        }
                        out.push(BrowserPassword {
                            browser: browser.to_string(),
                            url,
                            username,
                            password,
                        });
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_file(&tmp);
}

/// Đọc & lọc mật khẩu đã lưu trong các trình duyệt. Chỉ gọi khi user đã bật.
#[tauri::command]
pub fn list_passwords(
    query: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<BrowserPassword>, String> {
    // Guard: chỉ chạy khi bật trong Settings
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let enabled = conn
            .query_row("SELECT value FROM settings WHERE key='enable_browser_passwords'", [], |r| r.get::<_, String>(0))
            .unwrap_or_default();
        if enabled != "true" {
            return Err("Tính năng đọc mật khẩu trình duyệt đang tắt — bật trong Settings để dùng".into());
        }
    }

    let mut all: Vec<BrowserPassword> = Vec::new();
    for b in browsers() {
        let Some(key) = master_key(&b.user_data) else { continue };
        // Quét các profile: Default, Profile 1..N và cả thư mục gốc (Opera)
        let mut dbs: Vec<PathBuf> = Vec::new();
        let root_db = b.user_data.join("Login Data");
        if root_db.exists() {
            dbs.push(root_db);
        }
        if let Ok(entries) = std::fs::read_dir(&b.user_data) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if name == "Default" || name.starts_with("Profile") {
                        let db = p.join("Login Data");
                        if db.exists() {
                            dbs.push(db);
                        }
                    }
                }
            }
        }
        for db in dbs {
            read_profile(b.name, &key, &db, &mut all);
        }
    }

    let q = query.trim().to_lowercase();
    if !q.is_empty() {
        all.retain(|p| {
            p.url.to_lowercase().contains(&q)
                || p.username.to_lowercase().contains(&q)
                || p.browser.to_lowercase().contains(&q)
        });
    }
    // Khử trùng lặp (nhiều profile lưu trùng)
    all.dedup_by(|a, b| a.url == b.url && a.username == b.username && a.password == b.password);
    all.truncate(50);
    Ok(all)
}

/// Copy một chuỗi bí mật vào clipboard KÈM format loại trừ để không lọt vào
/// clipboard history của Windows lẫn của HeaSpot.
#[tauri::command]
pub fn copy_secret(text: String) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{GlobalFree, HANDLE};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    const CF_UNICODETEXT: u32 = 13;
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len() * 2;

    unsafe {
        let mem = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if mem.is_null() {
            return Err("không cấp phát được bộ nhớ".into());
        }
        let ptr = GlobalLock(mem);
        if ptr.is_null() {
            GlobalFree(mem);
            return Err("không khóa được bộ nhớ".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, bytes);
        GlobalUnlock(mem);

        if OpenClipboard(std::ptr::null_mut()) == 0 {
            GlobalFree(mem);
            return Err("clipboard đang bị giữ".into());
        }
        EmptyClipboard();
        SetClipboardData(CF_UNICODETEXT, mem as HANDLE);
        // Đánh dấu loại trừ khỏi mọi bộ theo dõi clipboard
        let name: Vec<u16> = "ExcludeClipboardContentFromMonitorProcessing"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let excl = RegisterClipboardFormatW(name.as_ptr());
        if excl != 0 {
            let dummy = GlobalAlloc(GMEM_MOVEABLE, 1);
            if !dummy.is_null() {
                SetClipboardData(excl, dummy as HANDLE);
            }
        }
        CloseClipboard();
    }
    Ok(())
}
