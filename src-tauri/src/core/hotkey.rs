use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::sync::RwLock;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Alt+Space bị app khác chiếm (VD: PowerToys Run) -> chuyển sang bắt bằng LL hook
static ALT_SPACE_VIA_HOOK: AtomicBool = AtomicBool::new(false);
static WIN_V_ENABLED: AtomicBool = AtomicBool::new(true);

fn config() -> &'static RwLock<(String, String)> {
    static CONFIG: OnceLock<RwLock<(String, String)>> = OnceLock::new();
    CONFIG.get_or_init(|| RwLock::new(("Alt+Space".into(), "Win+V".into())))
}

fn shortcut_for(value: &str) -> Option<Shortcut> {
    match value {
        "Alt+Space" => Some(Shortcut::new(Some(Modifiers::ALT), Code::Space)),
        "Ctrl+Space" => Some(Shortcut::new(Some(Modifiers::CONTROL), Code::Space)),
        "Ctrl+Alt+Space" => Some(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space)),
        "Alt+F1" => Some(Shortcut::new(Some(Modifiers::ALT), Code::F1)),
        "Ctrl+Shift+V" => Some(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV)),
        "Alt+V" => Some(Shortcut::new(Some(Modifiers::ALT), Code::KeyV)),
        "Ctrl+Alt+V" => Some(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyV)),
        "Win+V" => None,
        _ => None,
    }
}

/// Plugin global-shortcut xử lý Alt+Space và Ctrl+Shift+V.
/// Riêng Win+V dùng low-level keyboard hook (xem bên dưới) vì shell của
/// Windows luôn giành phím này trước cơ chế RegisterHotKey thông thường.
pub fn build_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let current = config().read().ok().map(|c| c.clone()).unwrap_or_else(|| ("Alt+Space".into(), "Win+V".into()));
            if shortcut_for(&current.0).as_ref() == Some(shortcut) {
                crate::core::window::toggle(app, "search");
            } else if shortcut_for(&current.1).as_ref() == Some(shortcut)
                || (current.1 == "Win+V" && shortcut_for("Ctrl+Shift+V").as_ref() == Some(shortcut)) {
                crate::core::window::toggle(app, "clipboard");
            }
        })
        .build()
}

/// Đăng ký Alt+Space (search) và Ctrl+Shift+V (clipboard, phím phụ).
/// KHÔNG được panic: nếu Alt+Space bị app khác giữ (PowerToys Run...),
/// fallback sang low-level hook — hook chặn phím trước mọi app nên vẫn hoạt động.
pub fn register_shortcuts(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let settings = {
        let state = app.state::<crate::AppState>();
        let conn = state.db.lock().map_err(|e| std::io::Error::other(e.to_string()))?;
        crate::commands::settings::load(&conn)
    };
    apply_hotkeys(app, &settings.search_hotkey, &settings.clipboard_hotkey)
        .map_err(|e| std::io::Error::other(e).into())
}

pub fn apply_hotkeys(app: &AppHandle, search: &str, clipboard: &str) -> Result<(), String> {
    let search_shortcut = shortcut_for(search).ok_or("Hotkey launcher không hợp lệ")?;
    if clipboard != "Win+V" && shortcut_for(clipboard).is_none() { return Err("Hotkey clipboard không hợp lệ".into()); }
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| e.to_string())?;
    if let Err(e) = gs.register(search_shortcut) {
        if search == "Alt+Space" {
            ALT_SPACE_VIA_HOOK.store(true, Ordering::Relaxed);
        } else { return Err(format!("Hotkey launcher đang bị ứng dụng khác chiếm: {e}")); }
    } else {
        ALT_SPACE_VIA_HOOK.store(false, Ordering::Relaxed);
    }
    if let Some(sc) = shortcut_for(clipboard) {
        gs.register(sc).map_err(|e| format!("Hotkey clipboard đang bị chiếm: {e}"))?;
    } else if clipboard == "Win+V" {
        // Giữ Ctrl+Shift+V làm phím dự phòng khi Win+V bị policy/hook của hệ thống chặn.
        let _ = gs.register(shortcut_for("Ctrl+Shift+V").unwrap());
    }
    WIN_V_ENABLED.store(clipboard == "Win+V", Ordering::Relaxed);
    if let Ok(mut c) = config().write() { *c = (search.to_string(), clipboard.to_string()); }
    Ok(())
}

// ---------------------------------------------------------------------------
// Win+V qua WH_KEYBOARD_LL: chặn phím TRƯỚC khi shell Windows xử lý,
// nuốt sự kiện để panel clipboard mặc định không hiện (cách của PowerToys).
// ---------------------------------------------------------------------------

static APP: OnceLock<AppHandle> = OnceLock::new();

/// Cài low-level keyboard hook trên thread riêng có message loop
pub fn install_winv_hook(app: AppHandle) {
    let _ = APP.set(app);
    std::thread::spawn(|| unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, MSG,
            WH_KEYBOARD_LL,
        };
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(winv_proc), std::ptr::null_mut(), 0);
        if hook.is_null() {
            eprintln!("không cài được keyboard hook cho Win+V");
            return;
        }
        // Hook callback cần message loop trên thread này
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn winv_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, GetAsyncKeyState, KEYEVENTF_KEYUP, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN,
        VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_SYSKEYDOWN,
    };

    const VK_V: u32 = 0x56;
    const VK_SPACE_CODE: u32 = 0x20;

    if code == HC_ACTION as i32 {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        let is_down = wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN;
        let held = |vk: u16| (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0;

        // Win+V -> Clipboard Manager
        if is_down && kb.vkCode == VK_V {
            let win_held = held(VK_LWIN) || held(VK_RWIN);
            let other_mod = held(VK_CONTROL) || held(VK_MENU) || held(VK_SHIFT);
            if win_held && !other_mod && WIN_V_ENABLED.load(Ordering::Relaxed) {
                // Gửi một phím "rỗng" (0xFF) để shell không mở Start Menu
                // khi người dùng nhả phím Win sau đó
                keybd_event(0xFF, 0, 0, 0);
                keybd_event(0xFF, 0, KEYEVENTF_KEYUP, 0);
                if let Some(app) = APP.get() {
                    crate::core::window::toggle(app, "clipboard");
                }
                return 1; // nuốt sự kiện -> panel clipboard của Windows không hiện
            }
        }

        // Alt+Space -> Search (chỉ khi RegisterHotKey thất bại vì app khác chiếm)
        if is_down
            && kb.vkCode == VK_SPACE_CODE
            && ALT_SPACE_VIA_HOOK.load(Ordering::Relaxed)
        {
            let alt_held = held(VK_MENU);
            let other_mod =
                held(VK_CONTROL) || held(VK_SHIFT) || held(VK_LWIN) || held(VK_RWIN);
            if alt_held && !other_mod {
                if let Some(app) = APP.get() {
                    crate::core::window::toggle(app, "search");
                }
                return 1; // nuốt -> PowerToys Run/system menu không nhận được
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}
