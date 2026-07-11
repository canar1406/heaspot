use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Alt+Space bị app khác chiếm (VD: PowerToys Run) -> chuyển sang bắt bằng LL hook
static ALT_SPACE_VIA_HOOK: AtomicBool = AtomicBool::new(false);

fn alt_space() -> Shortcut {
    Shortcut::new(Some(Modifiers::ALT), Code::Space)
}

fn ctrl_shift_v() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV)
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
            if shortcut == &alt_space() {
                crate::core::window::toggle(app, "search");
            } else if shortcut == &ctrl_shift_v() {
                crate::core::window::toggle(app, "clipboard");
            }
        })
        .build()
}

/// Đăng ký Alt+Space (search) và Ctrl+Shift+V (clipboard, phím phụ).
/// KHÔNG được panic: nếu Alt+Space bị app khác giữ (PowerToys Run...),
/// fallback sang low-level hook — hook chặn phím trước mọi app nên vẫn hoạt động.
pub fn register_shortcuts(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let gs = app.global_shortcut();
    if let Err(e) = gs.register(alt_space()) {
        eprintln!("Alt+Space bị app khác giữ ({e}) -> dùng keyboard hook");
        ALT_SPACE_VIA_HOOK.store(true, Ordering::Relaxed);
    }
    if let Err(e) = gs.register(ctrl_shift_v()) {
        eprintln!("không đăng ký được Ctrl+Shift+V: {e}");
    }
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
            if win_held && !other_mod {
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
