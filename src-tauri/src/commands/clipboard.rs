//! WinSpot Clipboard (plan.md): Alfred UX + Pin/Preview của Windows.
//! - Watcher đa định dạng: text, link, ảnh (PNG + thumbnail), danh sách file
//! - Privacy Guard: bỏ qua password manager & format ExcludeClipboardContentFromMonitorProcessing
//! - Pin, auto-cleanup (giữ 500 item chưa pin), source app
//! - Auto-paste: ghi clipboard -> focus lại cửa sổ cũ -> giả lập Ctrl+V

use base64::Engine;
use md5::Digest;
use rusqlite::params;
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize)]
pub struct ClipItem {
    pub id: i64,
    pub content: String,
    pub kind: String, // "text" | "link" | "image" | "files"
    pub created_at: String,
    pub pinned: bool,
    pub source_app: String,
    pub thumb: String, // data URL thumbnail cho ảnh
}

static SUPPRESS_WATCHER_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

pub fn suppress_watcher_for(duration_ms: u64) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64).unwrap_or(0);
    SUPPRESS_WATCHER_UNTIL_MS.store(now.saturating_add(duration_ms), Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// Watcher
// ---------------------------------------------------------------------------

pub fn spawn_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let Ok(conn) = crate::db::open_conn() else {
            return;
        };
        let Ok(mut cb) = arboard::Clipboard::new() else {
            return;
        };
        let mut policy = crate::commands::settings::load(&conn);
        prune_clip_history(&conn, policy.max_clipboard_items, policy.clipboard_retention_days);
        cleanup_clip_cache(&conn);
        let mut policy_ticks: u8 = 0;
        let mut last_text: Option<String> = conn
            .query_row(
                "SELECT content FROM clipboard WHERE kind IN ('text','link') ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .ok();
        let mut last_image_hash: Option<String> = None;
        let mut last_files: Option<String> = None;

        loop {
            std::thread::sleep(Duration::from_millis(700));
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64).unwrap_or(0);
            if now < SUPPRESS_WATCHER_UNTIL_MS.load(Ordering::Relaxed) { continue; }
            policy_ticks = policy_ticks.saturating_add(1);
            if policy_ticks >= 30 {
                policy = crate::commands::settings::load(&conn);
                prune_clip_history(&conn, policy.max_clipboard_items, policy.clipboard_retention_days);
                policy_ticks = 0;
            }

            // Privacy Guard: format loại trừ hoặc copy từ password manager
            if unsafe { clipboard_excluded() } {
                continue;
            }
            let source = foreground_process_name();
            let source_l = source.to_lowercase();
            if policy.privacy_apps.split([',', ';', '\n']).map(str::trim).filter(|s| !s.is_empty())
                .any(|app_name| source_l.contains(&app_name.to_lowercase())) {
                continue;
            }

            let mut inserted = false;

            // 1. Danh sách file (copy file trong Explorer)
            if let Some(files) = unsafe { read_file_list() } {
                let joined = files.join("\n");
                if last_files.as_deref() != Some(&joined) {
                    let _ = conn.execute(
                        "INSERT INTO clipboard (content, kind, source_app) VALUES (?1, 'files', ?2)",
                        params![joined, source],
                    );
                    last_files = Some(joined);
                    inserted = true;
                }
            }
            // 2. Ảnh (chụp màn hình, copy ảnh)
            else if let Ok(img) = cb.get_image() {
                let hash = format!("{:x}", md5::Md5::digest(&img.bytes));
                if last_image_hash.as_deref() != Some(&hash) {
                    if let Some((path, thumb, label)) = save_image(&img) {
                        let _ = conn.execute(
                            "INSERT INTO clipboard (content, kind, source_app, thumb) \
                             VALUES (?1, 'image', ?2, ?3)",
                            params![path, source, thumb],
                        );
                        // content = đường dẫn PNG; preview label nằm trong created_at? không —
                        // label WxH đã nhúng trong thumb subtitle phía UI
                        let _ = label;
                        inserted = true;
                    }
                    last_image_hash = Some(hash);
                }
            }
            // 3. Text / Link
            else if let Ok(text) = cb.get_text() {
                if !text.trim().is_empty()
                    && text.len() <= 100_000
                    && last_text.as_deref() != Some(text.as_str())
                {
                    let kind = if text.starts_with("http://") || text.starts_with("https://") {
                        "link"
                    } else {
                        "text"
                    };
                    let _ = conn.execute(
                        "INSERT INTO clipboard (content, kind, source_app) VALUES (?1, ?2, ?3)",
                        params![text, kind, source],
                    );
                    last_text = Some(text);
                    inserted = true;
                }
            }

            if inserted {
                // Auto-cleanup: giữ tối đa MAX_UNPINNED item chưa pin
                prune_clip_history(&conn, policy.max_clipboard_items, policy.clipboard_retention_days);
                // Báo UI update real-time nếu đang mở
                let _ = app.emit("clipboard://changed", ());
            }
        }
    });
}

/// Lưu ảnh RGBA thành PNG trong %APPDATA%\winspot\clips + tạo thumbnail base64
fn save_image(img: &arboard::ImageData) -> Option<(String, String, String)> {
    let (w, h) = (img.width as u32, img.height as u32);
    let rgba = image::RgbaImage::from_raw(w, h, img.bytes.to_vec())?;

    let name = format!(
        "clip_{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis()
    );
    let path = crate::db::clips_dir().join(&name);
    rgba.save_with_format(&path, image::ImageFormat::Png).ok()?;

    // Thumbnail cao tối đa 64px
    let scale = 64.0 / h.max(1) as f32;
    let (tw, th) = if scale < 1.0 {
        (((w as f32 * scale) as u32).max(1), 64)
    } else {
        (w, h)
    };
    let thumb_img = image::imageops::thumbnail(&rgba, tw, th);
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(thumb_img)
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .ok()?;
    let thumb = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(buf)
    );

    Some((
        path.to_string_lossy().to_string(),
        thumb,
        format!("Ảnh {w}×{h}"),
    ))
}

/// Format bảo mật chuẩn Windows: app yêu cầu KHÔNG lưu clipboard này
unsafe fn clipboard_excluded() -> bool {
    use std::sync::OnceLock;
    use windows_sys::Win32::System::DataExchange::{
        IsClipboardFormatAvailable, RegisterClipboardFormatW,
    };
    static FMT: OnceLock<u32> = OnceLock::new();
    let fmt = *FMT.get_or_init(|| {
        let name: Vec<u16> = "ExcludeClipboardContentFromMonitorProcessing"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        RegisterClipboardFormatW(name.as_ptr())
    });
    fmt != 0 && IsClipboardFormatAvailable(fmt) != 0
}

/// Tên process của cửa sổ đang focus (VD: "chrome") — lưu làm source_app
fn foreground_process_name() -> String {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId,
        };
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return String::new();
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(fg, &mut pid);
        if pid == 0 {
            return String::new();
        }
        crate::plugins::window_walker::process_path_of(pid)
            .and_then(|p| {
                std::path::Path::new(&p)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
            })
            .unwrap_or_default()
    }
}

/// Đọc CF_HDROP (danh sách file được copy trong Explorer)
unsafe fn read_file_list() -> Option<Vec<String>> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::UI::Shell::DragQueryFileW;

    const CF_HDROP: u32 = 15;
    if IsClipboardFormatAvailable(CF_HDROP) == 0 {
        return None;
    }
    if OpenClipboard(std::ptr::null_mut()) == 0 {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    let h = GetClipboardData(CF_HDROP);
    if !h.is_null() {
        let hdrop = h as windows_sys::Win32::UI::Shell::HDROP;
        let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, std::ptr::null_mut(), 0);
        for i in 0..count {
            let len = DragQueryFileW(hdrop, i, std::ptr::null_mut(), 0);
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u16; (len + 1) as usize];
            let got = DragQueryFileW(hdrop, i, buf.as_mut_ptr(), buf.len() as u32);
            if got > 0 {
                out.push(String::from_utf16_lossy(&buf[..got as usize]));
            }
        }
    }
    CloseClipboard();
    (!out.is_empty()).then_some(out)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_clipboard_history(
    limit: Option<u32>,
    filter: Option<String>,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<ClipItem>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(50);
    let pattern = format!("%{}%", filter.unwrap_or_default());
    let mut stmt = conn
        .prepare(
            "SELECT id, content, kind, created_at, is_pinned, source_app, thumb FROM clipboard \
             WHERE content LIKE ?1 OR source_app LIKE ?1 \
             ORDER BY is_pinned DESC, id DESC LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![pattern, limit], |r| {
            Ok(ClipItem {
                id: r.get(0)?,
                content: r.get(1)?,
                kind: r.get(2)?,
                created_at: r.get(3)?,
                pinned: r.get::<_, i64>(4)? != 0,
                source_app: r.get(5)?,
                thumb: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

/// Ghim / bỏ ghim — item pin không bị Clear All và auto-cleanup xoá
#[tauri::command]
pub fn toggle_pin(id: i64, state: tauri::State<'_, crate::AppState>) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE clipboard SET is_pinned = 1 - is_pinned WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT is_pinned FROM clipboard WHERE id = ?1",
        params![id],
        |r| r.get::<_, i64>(0).map(|v| v != 0),
    )
    .map_err(|e| e.to_string())
}

/// Sửa nội dung (tự động lưu từ preview pane)
#[tauri::command]
pub fn update_clipboard_item(
    id: i64,
    content: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE clipboard SET content = ?1 WHERE id = ?2",
        params![content, id],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Copy lại vào clipboard (không auto-paste)
#[tauri::command]
pub fn copy_clipboard_item(
    id: i64,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let (content, kind) = fetch_item(&state, id)?;
    write_clipboard(&content, &kind)
}

/// AUTO-PASTE: ghi item vào clipboard, focus lại cửa sổ trước đó, giả lập Ctrl+V.
/// `plain` = dán dạng văn bản thuần (với dữ liệu text hiện tại là tương đương).
#[tauri::command]
pub fn paste_clipboard_item(
    id: i64,
    plain: Option<bool>,
    state: tauri::State<'_, crate::AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let (content, kind) = fetch_item(&state, id)?;

    // Ẩn WinSpot trước để focus trả về cửa sổ cũ
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    let _ = app.emit("winspot://hidden", ());
    crate::core::window::trim_memory();

    let effective_kind = if plain.unwrap_or(false) && (kind == "image" || kind == "files") {
        // Plain paste ảnh/file -> dán đường dẫn thay vì dữ liệu nhị phân/CF_HDROP.
        "text".to_string()
    } else {
        kind
    };
    write_clipboard(&content, &effective_kind)?;

    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP};
        use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

        let prev = crate::core::window::prev_foreground();
        if prev != 0 {
            SetForegroundWindow(prev as _);
        }
        std::thread::sleep(Duration::from_millis(120));

        const VK_CONTROL: u8 = 0x11;
        const VK_V: u8 = 0x56;
        keybd_event(VK_CONTROL, 0, 0, 0);
        keybd_event(VK_V, 0, 0, 0);
        keybd_event(VK_V, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0);
    }
    Ok(())
}

fn fetch_item(
    state: &tauri::State<'_, crate::AppState>,
    id: i64,
) -> Result<(String, String), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT content, kind FROM clipboard WHERE id = ?1",
        params![id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )
    .map_err(|e| e.to_string())
}

fn write_clipboard(content: &str, kind: &str) -> Result<(), String> {
    if kind == "files" {
        return write_file_list_clipboard(content);
    }
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    if kind == "image" {
        let img = image::open(content).map_err(|e| e.to_string())?.to_rgba8();
        let (w, h) = img.dimensions();
        cb.set_image(arboard::ImageData {
            width: w as usize,
            height: h as usize,
            bytes: std::borrow::Cow::Owned(img.into_raw()),
        })
        .map_err(|e| e.to_string())
    } else {
        cb.set_text(content.to_string()).map_err(|e| e.to_string())
    }
}

/// Dán trực tiếp một đoạn text vào cửa sổ đang active trước WinSpot.
#[tauri::command]
pub fn paste_text(text: String, app: AppHandle) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("nội dung trống".into());
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    let _ = app.emit("winspot://hidden", ());
    write_clipboard(&text, "text")?;
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP};
        use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
        let prev = crate::core::window::prev_foreground();
        if prev != 0 { SetForegroundWindow(prev as _); }
        std::thread::sleep(Duration::from_millis(120));
        keybd_event(0x11, 0, 0, 0);
        keybd_event(0x56, 0, 0, 0);
        keybd_event(0x56, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(0x11, 0, KEYEVENTF_KEYUP, 0);
    }
    Ok(())
}

/// Ghi danh sách đường dẫn thành CF_HDROP để Explorer/app đích nhận đúng file thật.
fn write_file_list_clipboard(content: &str) -> Result<(), String> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    const CF_HDROP: u32 = 15;
    let paths: Vec<&str> = content.lines().map(str::trim).filter(|p| !p.is_empty()).collect();
    if paths.is_empty() { return Err("danh sách file trống".into()); }

    // DROPFILES header (20 byte), theo sau là chuỗi UTF-16 multi-string kết thúc bằng hai NUL.
    let mut data = vec![0u8; 20];
    data[0..4].copy_from_slice(&20u32.to_le_bytes());
    data[16..20].copy_from_slice(&1u32.to_le_bytes()); // fWide = TRUE
    let mut wide: Vec<u16> = Vec::new();
    for path in paths {
        wide.extend(path.encode_utf16());
        wide.push(0);
    }
    wide.push(0);
    let wide_bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2) };
    data.extend_from_slice(wide_bytes);

    unsafe {
        let mem = GlobalAlloc(GMEM_MOVEABLE, data.len());
        if mem.is_null() { return Err("không cấp phát được bộ nhớ clipboard".into()); }
        let ptr = GlobalLock(mem);
        if ptr.is_null() { GlobalFree(mem); return Err("không khóa được bộ nhớ clipboard".into()); }
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
        GlobalUnlock(mem);
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(std::ptr::null_mut()) != 0 { opened = true; break; }
            std::thread::sleep(Duration::from_millis(20));
        }
        if !opened { GlobalFree(mem); return Err("clipboard đang bị ứng dụng khác giữ".into()); }
        EmptyClipboard();
        let result = SetClipboardData(CF_HDROP, mem as _);
        CloseClipboard();
        if result.is_null() { GlobalFree(mem); return Err("không ghi được danh sách file".into()); }
    }
    Ok(())
}

fn prune_clip_history(conn: &rusqlite::Connection, max_unpinned: u32, retention_days: u32) {
    if retention_days > 0 {
        let modifier = format!("-{retention_days} days");
        if let Ok(mut stmt) = conn.prepare(
            "SELECT content FROM clipboard WHERE kind = 'image' AND is_pinned = 0 \
             AND created_at < datetime('now', 'localtime', ?1)",
        ) {
            let paths: Vec<String> = stmt.query_map(params![modifier], |r| r.get(0))
                .map(|rows| rows.flatten().collect()).unwrap_or_default();
            drop(stmt);
            for path in paths { let _ = std::fs::remove_file(path); }
        }
        let modifier = format!("-{retention_days} days");
        let _ = conn.execute(
            "DELETE FROM clipboard WHERE is_pinned = 0 AND created_at < datetime('now', 'localtime', ?1)",
            params![modifier],
        );
    }
    let sql = "SELECT content FROM clipboard WHERE kind = 'image' AND is_pinned = 0 AND id NOT IN \
               (SELECT id FROM clipboard WHERE is_pinned = 0 ORDER BY id DESC LIMIT ?1)";
    if let Ok(mut stmt) = conn.prepare(sql) {
        let paths: Vec<String> = stmt.query_map(params![max_unpinned], |r| r.get(0))
            .map(|rows| rows.flatten().collect()).unwrap_or_default();
        drop(stmt);
        for path in paths { let _ = std::fs::remove_file(path); }
    }
    let _ = conn.execute(
        "DELETE FROM clipboard WHERE is_pinned = 0 AND id NOT IN \
         (SELECT id FROM clipboard WHERE is_pinned = 0 ORDER BY id DESC LIMIT ?1)",
        params![max_unpinned],
    );
}

/// Xóa các PNG mồ côi còn sót sau crash/nâng cấp; chỉ giữ file còn được DB tham chiếu.
fn cleanup_clip_cache(conn: &rusqlite::Connection) {
    use std::collections::HashSet;
    let referenced: HashSet<PathBuf> = conn.prepare("SELECT content FROM clipboard WHERE kind = 'image'")
        .and_then(|mut s| s.query_map([], |r| r.get::<_, String>(0)).map(|rows| rows.flatten().map(PathBuf::from).collect()))
        .unwrap_or_default();
    if let Ok(entries) = std::fs::read_dir(crate::db::clips_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && !referenced.contains(&path) { let _ = std::fs::remove_file(path); }
        }
    }
}

/// Ảnh full-size cho preview pane (data URL)
#[tauri::command]
pub fn get_clip_image(id: i64, state: tauri::State<'_, crate::AppState>) -> Result<String, String> {
    let (content, kind) = fetch_item(&state, id)?;
    if kind != "image" {
        return Err("không phải ảnh".into());
    }
    let bytes = std::fs::read(&content).map_err(|e| e.to_string())?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[tauri::command]
pub fn delete_clipboard_item(
    id: i64,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    // Xoá luôn file ảnh cache nếu có
    if let Ok((content, kind)) = conn.query_row(
        "SELECT content, kind FROM clipboard WHERE id = ?1",
        params![id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    ) {
        if kind == "image" {
            let _ = std::fs::remove_file(PathBuf::from(content));
        }
    }
    conn.execute("DELETE FROM clipboard WHERE id = ?1", params![id])
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Clear cache (Settings): xoá TOÀN BỘ cache của app trong RAM + ảnh clip mồ côi
/// trên đĩa. Gom mọi cache tra cứu của mọi tính năng, không riêng clipboard.
/// KHÔNG đụng vào clipboard history / snippets / settings của người dùng.
#[tauri::command]
pub fn clear_cache(state: tauri::State<'_, crate::AppState>) -> Result<String, String> {
    use std::collections::HashSet;
    crate::commands::knowledge::clear_caches(); // dịch, tra nhanh, wiki, Google
    crate::commands::search::clear_fulltext_cache(); // full-text hybrid Windows Search/Everything
    crate::commands::capacities::clear_spaces_cache(); // spaceIds Capacities
    crate::core::indexer::clear_icon_cache(); // icon app/file

    let referenced: HashSet<PathBuf> = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.prepare("SELECT content FROM clipboard WHERE kind = 'image'")
            .and_then(|mut s| {
                s.query_map([], |r| r.get::<_, String>(0))
                    .map(|rows| rows.flatten().map(PathBuf::from).collect())
            })
            .unwrap_or_default()
    };
    let mut freed: u64 = 0;
    let mut count = 0u32;
    if let Ok(entries) = std::fs::read_dir(crate::db::clips_dir()) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() && !referenced.contains(&p) {
                if let Ok(meta) = e.metadata() {
                    freed += meta.len();
                }
                if std::fs::remove_file(&p).is_ok() {
                    count += 1;
                }
            }
        }
    }
    let mb = freed as f64 / 1_048_576.0;
    Ok(format!(
        "Đã dọn toàn bộ cache: dịch, tra nhanh, Wikipedia, Google, full-text, Capacities, icon + {count} ảnh tạm ({mb:.1} MB)."
    ))
}

/// Clear All — GIỮ LẠI các item đã pin (học từ Windows Clipboard)
#[tauri::command]
pub fn clear_clipboard_history(state: tauri::State<'_, crate::AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    // Dọn file ảnh của các item sắp xoá
    let mut stmt = conn
        .prepare("SELECT content FROM clipboard WHERE is_pinned = 0 AND kind = 'image'")
        .map_err(|e| e.to_string())?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    drop(stmt);
    for p in paths {
        let _ = std::fs::remove_file(PathBuf::from(p));
    }
    conn.execute("DELETE FROM clipboard WHERE is_pinned = 0", [])
        .map(|_| ())
        .map_err(|e| e.to_string())
}
