use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

/// Cửa sổ đang focus TRƯỚC khi WinSpot hiện — dùng cho auto-paste (Ctrl+V)
static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);
static RESIZE_GENERATION: AtomicU64 = AtomicU64::new(0);
static FOCUS_GRACE_UNTIL_MS: AtomicU64 = AtomicU64::new(0);
static VISIBILITY_GENERATION: AtomicU64 = AtomicU64::new(0);
static MAIN_VISIBLE_INTENT: AtomicBool = AtomicBool::new(false);
static MAIN_FOCUS_ACQUIRED: AtomicBool = AtomicBool::new(false);

fn visibility_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn monotonic_millis() -> u64 {
    use std::time::Instant;

    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn begin_focus_grace(duration_ms: u64) {
    FOCUS_GRACE_UNTIL_MS.store(
        monotonic_millis().saturating_add(duration_ms),
        Ordering::Relaxed,
    );
}

fn focus_grace_active() -> bool {
    monotonic_millis() < FOCUS_GRACE_UNTIL_MS.load(Ordering::Relaxed)
}

fn begin_show_request() -> u64 {
    let generation = VISIBILITY_GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    MAIN_VISIBLE_INTENT.store(true, Ordering::Release);
    MAIN_FOCUS_ACQUIRED.store(false, Ordering::Release);
    begin_focus_grace(700);
    generation
}

fn show_request_matches(request: u64, current: u64, visible_intent: bool) -> bool {
    visible_intent && request == current
}

fn show_request_is_active(generation: u64) -> bool {
    show_request_matches(
        generation,
        VISIBILITY_GENERATION.load(Ordering::Acquire),
        MAIN_VISIBLE_INTENT.load(Ordering::Acquire),
    )
}

fn fallback_should_activate(
    visible: bool,
    focus_was_acquired: bool,
    foreground_is_ours: bool,
) -> bool {
    !visible || (!focus_was_acquired && !foreground_is_ours)
}

fn cancel_show_requests() {
    MAIN_VISIBLE_INTENT.store(false, Ordering::Release);
    MAIN_FOCUS_ACQUIRED.store(false, Ordering::Release);
    VISIBILITY_GENERATION.fetch_add(1, Ordering::AcqRel);
    FOCUS_GRACE_UNTIL_MS.store(0, Ordering::Release);
}

pub fn prev_foreground() -> isize {
    PREV_FOREGROUND.load(Ordering::Relaxed)
}

/// Windows can occasionally miss WebView/Tauri focus notifications (notably
/// while an always-on-top, borderless window is being activated).  Treat the
/// native foreground process as the source of truth instead of relying only
/// on `WindowEvent::Focused(false)`.
#[cfg(target_os = "windows")]
pub fn foreground_belongs_to_current_process() -> bool {
    unsafe {
        use windows_sys::Win32::System::Threading::GetCurrentProcessId;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId,
        };

        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return false;
        }
        let mut process_id = 0u32;
        GetWindowThreadProcessId(foreground, &mut process_id);
        process_id == GetCurrentProcessId()
    }
}

#[cfg(not(target_os = "windows"))]
pub fn foreground_belongs_to_current_process() -> bool {
    true
}

#[cfg(target_os = "windows")]
fn native_window_is_visible(win: &WebviewWindow) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;

    win.hwnd()
        .map(|hwnd| unsafe { IsWindowVisible(hwnd.0) != 0 })
        .unwrap_or_else(|_| win.is_visible().unwrap_or(false))
}

#[cfg(target_os = "windows")]
fn native_window_is_foreground(win: &WebviewWindow) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, GetForegroundWindow, GA_ROOT};

    win.hwnd()
        .map(|hwnd| unsafe {
            let foreground = GetForegroundWindow();
            !foreground.is_null()
                && (foreground == hwnd.0 || GetAncestor(foreground, GA_ROOT) == hwnd.0)
        })
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn native_window_is_foreground(_win: &WebviewWindow) -> bool {
    true
}

#[cfg(target_os = "windows")]
fn native_escape_was_pressed() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};

    // The low bit reports a press since the previous query. Query on every
    // watchdog sample (even while hidden) so an Escape used in another app
    // cannot remain queued and close HeaSpot the next time it opens.
    unsafe { (GetAsyncKeyState(VK_ESCAPE as i32) as u16 & 1) != 0 }
}

#[cfg(not(target_os = "windows"))]
fn native_escape_was_pressed() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn native_hide_window(win: &WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

    if let Ok(hwnd) = win.hwnd() {
        unsafe {
            ShowWindow(hwnd.0, SW_HIDE);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn native_hide_window(_win: &WebviewWindow) {}

#[cfg(not(target_os = "windows"))]
fn native_window_is_visible(win: &WebviewWindow) -> bool {
    win.is_visible().unwrap_or(false)
}

/// Use the native HWND for focus-sensitive keyboard handling. Tauri's cached
/// visibility/focus state can briefly disagree with an always-on-top WebView.
#[cfg(target_os = "windows")]
pub fn main_is_foreground(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .is_some_and(|win| native_window_is_foreground(&win))
}

#[cfg(not(target_os = "windows"))]
pub fn main_is_foreground(_app: &AppHandle) -> bool {
    true
}

/// Focus-loss events can be emitted while Windows is still transferring
/// activation from Start/another app to the launcher. During that short grace
/// period the retry path owns focus; afterwards a real external focus change
/// must hide the launcher.
fn should_hide_for_focus_state(
    foreground_is_ours: bool,
    focus_was_acquired: bool,
    grace_active: bool,
) -> bool {
    !foreground_is_ours && (focus_was_acquired || !grace_active)
}

pub fn should_hide_for_focus_loss() -> bool {
    should_hide_for_focus_state(
        foreground_belongs_to_current_process(),
        MAIN_FOCUS_ACQUIRED.load(Ordering::Acquire),
        focus_grace_active(),
    )
}

#[cfg(target_os = "windows")]
fn native_activate_window(win: &WebviewWindow) {
    unsafe {
        use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
            ShowWindow, SW_SHOW,
        };

        let Ok(hwnd) = win.hwnd() else {
            return;
        };
        let target = hwnd.0;
        let foreground = GetForegroundWindow();
        let current_thread = GetCurrentThreadId();
        let mut ignored_pid = 0u32;
        let target_thread = GetWindowThreadProcessId(target, &mut ignored_pid);
        let foreground_thread = if foreground.is_null() {
            0
        } else {
            GetWindowThreadProcessId(foreground, &mut ignored_pid)
        };

        let attached_target = target_thread != 0
            && target_thread != current_thread
            && AttachThreadInput(current_thread, target_thread, 1) != 0;
        let attached_foreground = foreground_thread != 0
            && foreground_thread != current_thread
            && foreground_thread != target_thread
            && AttachThreadInput(current_thread, foreground_thread, 1) != 0;

        ShowWindow(target, SW_SHOW);
        BringWindowToTop(target);
        SetForegroundWindow(target);
        SetFocus(target);

        if attached_foreground {
            AttachThreadInput(current_thread, foreground_thread, 0);
        }
        if attached_target {
            AttachThreadInput(current_thread, target_thread, 0);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn native_activate_window(_win: &WebviewWindow) {}

fn activate_show_request(win: &WebviewWindow, generation: u64) {
    let _visibility_guard = visibility_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !show_request_is_active(generation) {
        return;
    }
    let _ = win.show();
    native_activate_window(win);
    let _ = win.set_focus();
    native_activate_window(win);

    if native_window_is_foreground(win) {
        MAIN_FOCUS_ACQUIRED.store(true, Ordering::Release);
        FOCUS_GRACE_UNTIL_MS.store(0, Ordering::Release);
    }

    // Escape/blur may cancel this request while native activation is in
    // progress. Never let that stale activation resurrect the launcher.
    if !show_request_is_active(generation) {
        native_hide_window(win);
        let _ = win.hide();
    }
}

/// Record a real Tauri focus event as soon as it arrives. This closes the
/// acquisition grace immediately, so a subsequent click into another process
/// hides the launcher instead of being ignored for the remainder of 700 ms.
pub fn mark_main_focused() {
    if MAIN_VISIBLE_INTENT.load(Ordering::Acquire) {
        MAIN_FOCUS_ACQUIRED.store(true, Ordering::Release);
        FOCUS_GRACE_UNTIL_MS.store(0, Ordering::Release);
    }
}

/// Hide the launcher exactly once and keep the frontend/native state in sync.
pub fn hide_main(app: &AppHandle) {
    // Cancel first: an already queued frontend animation/fallback is then
    // unable to show the window again after Escape or a real focus loss.
    cancel_show_requests();
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let _visibility_guard = visibility_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let was_visible = native_window_is_visible(&win);
    // Always call the native hide operation. In practice Tauri's cached
    // `is_visible()` can report false for this transparent borderless window
    // while Win32 still considers its HWND visible. `ShowWindow` also works
    // when Escape is handled on the low-level keyboard hook thread, where the
    // higher-level Tauri hide request can otherwise be delayed or dropped.
    native_hide_window(&win);
    let _ = win.hide();
    if was_visible {
        let _ = app.emit("winspot://hidden", ());
        trim_memory();
    }
}

/// Backstop for missed blur events. Two consecutive samples are required so a
/// normal foreground transition while showing the WebView cannot immediately
/// close it again.
pub fn spawn_focus_watchdog(app: AppHandle) {
    std::thread::spawn(move || {
        let mut lost_samples = 0u8;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(120));
            let escape_pressed = native_escape_was_pressed();
            let visible = app
                .get_webview_window("main")
                .map(|win| native_window_is_visible(&win))
                .unwrap_or(false);
            if visible && !MAIN_VISIBLE_INTENT.load(Ordering::Acquire) {
                hide_main(&app);
                lost_samples = 0;
                continue;
            }
            if visible && escape_pressed && main_is_foreground(&app) {
                hide_main(&app);
                lost_samples = 0;
                continue;
            }
            if !visible || foreground_belongs_to_current_process() {
                lost_samples = 0;
                continue;
            }
            if !should_hide_for_focus_loss() {
                lost_samples = 0;
                continue;
            }
            lost_samples = lost_samples.saturating_add(1);
            if lost_samples >= 2 {
                hide_main(&app);
                lost_samples = 0;
            }
        }
    });
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

    // Keep the independent uninstall observer in the lower-right corner so it
    // remains visible without covering the vendor's centered dialog.
    if let Some(progress) = app.get_webview_window("uninstall-progress") {
        if let Ok(Some(monitor)) = progress.current_monitor() {
            let msize = monitor.size();
            let mpos = monitor.position();
            let scale = monitor.scale_factor();
            let width = 480.0 * scale;
            let height = 270.0 * scale;
            let x = mpos.x + (msize.width as f64 - width - 24.0 * scale) as i32;
            let y = mpos.y + (msize.height as f64 - height - 72.0 * scale) as i32;
            let _ = progress.set_position(tauri::PhysicalPosition::new(x, y));
        }
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
    let visible = native_window_is_visible(&win);

    if visible && main_is_foreground(app) && mode == "search" && prefill.is_empty() {
        hide_main(app);
    } else {
        let generation = begin_show_request();
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
            if !show_request_is_active(generation) {
                return;
            }
            if fallback_should_activate(
                native_window_is_visible(&win2),
                MAIN_FOCUS_ACQUIRED.load(Ordering::Acquire),
                foreground_belongs_to_current_process(),
            ) {
                activate_show_request(&win2, generation);
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
        let start_w = current
            .as_ref()
            .map(|s| s.width as f64 / scale)
            .unwrap_or(w);
        let start_h = current
            .as_ref()
            .map(|s| s.height as f64 / scale)
            .unwrap_or(h);
        let generation = RESIZE_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        std::thread::spawn(move || {
            const STEPS: u32 = 10;
            for step in 1..=STEPS {
                if RESIZE_GENERATION.load(Ordering::Relaxed) != generation {
                    return;
                }
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
    hide_main(&app);
}

/// Frontend calls this after resetting its animation state. Native activation
/// is more reliable than WebView `setFocus()` alone, especially for a launcher
/// bound to the lone Windows key.
#[tauri::command]
pub fn show_and_focus_main(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let generation = VISIBILITY_GENERATION.load(Ordering::Acquire);
        activate_show_request(&win, generation);
    }
}

/// Restore the launcher after an intentional native workflow (currently OCR)
/// that temporarily hid it. Unlike a raw WebView `show()`, this creates a new
/// visibility generation and therefore cannot revive a canceled old request.
#[tauri::command]
pub fn restore_main_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let generation = begin_show_request();
        activate_show_request(&win, generation);
    }
}

/// System tray: chuột phải có menu Mở / Thoát
pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;

    let show = MenuItem::with_id(app, "show", "Mở HeaSpot", true, None::<&str>)?;
    let progress = MenuItem::with_id(
        app,
        "uninstall-progress",
        "Tiến trình gỡ cài đặt…",
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, "settings", "Cài đặt…", true, None::<&str>)?;
    let update = MenuItem::with_id(
        app,
        "check-update",
        "Kiểm tra cập nhật…",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Thoát", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &progress, &settings, &update, &quit])?;

    TrayIconBuilder::with_id("winspot-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("HeaSpot")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => toggle(app, "search"),
            "uninstall-progress" => crate::commands::system::show_current_uninstall(app),
            "settings" => open_settings(app),
            "check-update" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::commands::updater::check_for_updates(app, true).await;
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{fallback_should_activate, should_hide_for_focus_state, show_request_matches};

    #[test]
    fn focus_loss_hides_immediately_after_launcher_owned_foreground() {
        assert!(should_hide_for_focus_state(false, true, true));
        assert!(!should_hide_for_focus_state(true, true, false));
    }

    #[test]
    fn acquisition_grace_only_protects_a_window_that_never_gained_focus() {
        assert!(!should_hide_for_focus_state(false, false, true));
        assert!(should_hide_for_focus_state(false, false, false));
    }

    #[test]
    fn stale_prepare_or_fallback_cannot_reopen_after_hide() {
        assert!(show_request_matches(7, 7, true));
        assert!(!show_request_matches(7, 8, true));
        assert!(!show_request_matches(7, 7, false));
    }

    #[test]
    fn fallback_never_steals_focus_back_after_initial_acquisition() {
        assert!(!fallback_should_activate(true, true, false));
        assert!(fallback_should_activate(true, false, false));
        assert!(fallback_should_activate(false, true, false));
    }
}
