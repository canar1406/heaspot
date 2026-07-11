use std::sync::atomic::{AtomicIsize, Ordering};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

/// Cửa sổ đang focus TRƯỚC khi WinSpot hiện — dùng cho auto-paste (Ctrl+V)
static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

pub fn prev_foreground() -> isize {
    PREV_FOREGROUND.load(Ordering::Relaxed)
}

const WINDOW_WIDTH: f64 = 680.0;
const CLIPBOARD_WIDTH: f64 = 900.0;
const CLIPBOARD_HEIGHT: f64 = 520.0;
const SETTINGS_HEIGHT: f64 = 560.0;
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

/// Hiện / ẩn cửa sổ chính. `mode` = "search" | "clipboard"
pub fn toggle(app: &AppHandle, mode: &str) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let visible = win.is_visible().unwrap_or(false);

    if visible && mode == "search" {
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
            } else if mode == "settings" {
                (CLIPBOARD_WIDTH, SETTINGS_HEIGHT)
            } else {
                (WINDOW_WIDTH, BAR_HEIGHT)
            };
            let _ = win.set_size(tauri::LogicalSize::new(w, h));
            recenter_x(&win, w);
        }
        // KHÔNG show ngay: frontend reset UI về trạng thái opacity 0 xong
        // sẽ tự gọi win.show() -> không còn khung hình cũ lóe lên (hết giật)
        let _ = app.emit("winspot://prepare", mode);

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
        let _ = win.set_size(tauri::LogicalSize::new(w, h));
        recenter_x(&win, w);
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

    let show = MenuItem::with_id(app, "show", "Mở WinSpot", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Cài đặt…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Thoát", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &quit])?;

    TrayIconBuilder::with_id("winspot-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("WinSpot")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => toggle(app, "search"),
            "settings" => toggle(app, "settings"),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}
