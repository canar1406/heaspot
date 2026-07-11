use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

/// Cửa sổ đang focus TRƯỚC khi WinSpot hiện — dùng cho auto-paste (Ctrl+V)
static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);
static RESIZE_GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn prev_foreground() -> isize {
    PREV_FOREGROUND.load(Ordering::Relaxed)
}

// Giữ cùng chiều rộng cho search/detail để WebView không giật ngang khi đổi chế độ.
const WINDOW_WIDTH: f64 = 900.0;
const CLIPBOARD_WIDTH: f64 = 900.0;
const CLIPBOARD_HEIGHT: f64 = 520.0;
const BAR_HEIGHT: f64 = 72.0;

/// Đặt cửa sổ giữa màn hình theo chiều ngang, cao 18% từ mép trên.
/// KHÔNG dùng Mica/Acrylic: hiệu ứng đó tô mờ cả vùng cửa sổ hình chữ nhật
/// bên ngoài panel bo góc, gây "viền dư". Nền mờ do CSS của panel tự vẽ.
pub fn setup_window(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let win = app
        .get_webview_window("main")
        .ok_or("không tìm thấy cửa sổ main")?;

    if let Ok(Some(monitor)) = win.current_monitor() {
        let msize = monitor.size();
        let mpos = monitor.position();
        let scale = monitor.scale_factor();
        let w_phys = WINDOW_WIDTH * scale;
        let x = mpos.x + ((msize.width as f64 - w_phys) / 2.0) as i32;
        let y = mpos.y + (msize.height as f64 * 0.18) as i32;
        let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
    }

    Ok(())
}

/// Căn giữa cửa sổ theo chiều ngang với bề rộng logical cho trước (giữ nguyên y)
fn recenter_x(win: &WebviewWindow, logical_w: f64) {
    if let (Ok(Some(mon)), Ok(pos)) = (win.current_monitor(), win.outer_position()) {
        let scale = mon.scale_factor();
        let x = mon.position().x + ((mon.size().width as f64 - logical_w * scale) / 2.0) as i32;
        let _ = win.set_position(tauri::PhysicalPosition::new(x, pos.y));
    }
}

/// Hiện / ẩn cửa sổ chính. `mode` = "search" | "clipboard".
pub fn toggle(app: &AppHandle, mode: &str) {
    toggle_with(app, mode, "");
}

/// Như `toggle` nhưng mở launcher với sẵn một đoạn text (prefill trigger keyword).
pub fn toggle_with(app: &AppHandle, mode: &str, prefill: &str) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let visible = win.is_visible().unwrap_or(false);

    if visible && mode == "search" && prefill.is_empty() {
        let _ = win.hide();
        let _ = app.emit("winspot://hidden", ());
        trim_memory();
    } else {
        // Ghi nhớ cửa sổ đang focus để auto-paste sau này
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
            let fg = GetForegroundWindow();
            if !fg.is_null() {
                PREV_FOREGROUND.store(fg as isize, Ordering::Relaxed);
            }
        }
        // Đặt sẵn kích thước ĐÚNG trước khi hiện để không thấy cửa sổ "nhảy"
        if !visible {
            let (w, h) = if mode == "clipboard" {
                (CLIPBOARD_WIDTH, CLIPBOARD_HEIGHT)
            } else {
                (WINDOW_WIDTH, BAR_HEIGHT)
            };
            let _ = win.set_size(tauri::LogicalSize::new(w, h));
            recenter_x(&win, w);
        }
        // KHÔNG show ngay: frontend reset UI về trạng thái opacity 0 xong
        // sẽ tự gọi win.show() -> không còn khung hình cũ lóe lên (hết giật)
        let payload = serde_json::json!({ "mode": mode, "prefill": prefill });
        let _ = app.emit("winspot://prepare", payload);

        // Phòng hờ frontend chưa sẵn sàng (mới khởi động): show sau 350ms
        let win2 = win.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(350));
            if !win2.is_visible().unwrap_or(false) {
                let _ = win2.show();
                let _ = win2.set_focus();
            }
        });
    }
}

/// Mở cửa sổ Cài đặt (cửa sổ Windows riêng, có viền, hiện trên taskbar)
pub fn open_settings(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle) {
    open_settings(&app);
}

/// Giải phóng RAM khi app bị ẩn (working set trim)
pub fn trim_memory() {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::ProcessStatus::K32EmptyWorkingSet;
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        K32EmptyWorkingSet(GetCurrentProcess());
    }
}

/// Frontend gọi để cửa sổ co giãn theo nội dung. `width` tuỳ chọn
/// (mode clipboard dùng cửa sổ rộng hơn) — đổi width sẽ tự căn giữa lại.
#[tauri::command]
pub fn resize_window(app: AppHandle, height: f64, width: Option<f64>) {
    if let Some(win) = app.get_webview_window("main") {
        let w = width.unwrap_or(WINDOW_WIDTH).clamp(400.0, 1100.0);
        let h = height.clamp(BAR_HEIGHT, 640.0);
        let scale = win.scale_factor().unwrap_or(1.0);
        let current = win.outer_size().ok();
        let start_w = current.as_ref().map(|s| s.width as f64 / scale).unwrap_or(w);
        let start_h = current.as_ref().map(|s| s.height as f64 / scale).unwrap_or(h);
        let generation = RESIZE_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        std::thread::spawn(move || {
            const STEPS: u32 = 10;
            for step in 1..=STEPS {
                if RESIZE_GENERATION.load(Ordering::Relaxed) != generation { return; }
                let t = step as f64 / STEPS as f64;
                let eased = 1.0 - (1.0 - t).powi(3);
                let nw = start_w + (w - start_w) * eased;
                let nh = start_h + (h - start_h) * eased;
                let _ = win.set_size(tauri::LogicalSize::new(nw, nh));
                recenter_x(&win, nw);
                std::thread::sleep(std::time::Duration::from_millis(14));
            }
        });
    }
}

/// Frontend gọi khi bấm Esc: ẩn cửa sổ + giải phóng RAM
#[tauri::command]
pub fn hide_and_trim(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    let _ = app.emit("winspot://hidden", ());
    trim_memory();
}

/// System tray: chuột phải có menu Mở / Thoát
pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;

    let show = MenuItem::with_id(app, "show", "Mở HeaSpot", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Cài đặt…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Thoát", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &quit])?;

    TrayIconBuilder::with_id("winspot-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("HeaSpot")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => toggle(app, "search"),
            "settings" => open_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}
