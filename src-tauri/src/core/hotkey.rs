use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::sync::RwLock;
use std::{collections::HashMap, str::FromStr};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Alt+Space bị app khác chiếm (VD: PowerToys Run) -> chuyển sang bắt bằng LL hook
static ALT_SPACE_VIA_HOOK: AtomicBool = AtomicBool::new(false);
static WIN_V_ENABLED: AtomicBool = AtomicBool::new(true);

fn config() -> &'static RwLock<(String, String)> {
    static CONFIG: OnceLock<RwLock<(String, String)>> = OnceLock::new();
    CONFIG.get_or_init(|| RwLock::new(("Alt+Space".into(), "Win+V".into())))
}

#[derive(Clone)]
struct FeatureBinding {
    id: String,
    hotkey: String,
    keyword: String,
}

fn feature_bindings() -> &'static RwLock<Vec<FeatureBinding>> {
    static BINDINGS: OnceLock<RwLock<Vec<FeatureBinding>>> = OnceLock::new();
    BINDINGS.get_or_init(|| RwLock::new(Vec::new()))
}

fn shortcut_for(value: &str) -> Option<Shortcut> {
    if value.eq_ignore_ascii_case("Win+V") { return None; }
    Shortcut::from_str(value).ok()
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
            } else if shortcut_for(&current.1).as_ref() == Some(shortcut) {
                crate::core::window::toggle(app, "clipboard");
            } else {
                let binding = feature_bindings().read().ok()
                    .and_then(|items| items.iter().find(|b| shortcut_for(&b.hotkey).as_ref() == Some(shortcut)).cloned());
                if let Some(binding) = binding {
                    let app = app.clone();
                    std::thread::spawn(move || activate_feature_hotkey(app, binding));
                }
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
    apply_hotkeys(
        app,
        &settings.search_hotkey,
        &settings.clipboard_hotkey,
        &settings.feature_hotkeys,
        &settings.keywords,
    )
        .map_err(|e| std::io::Error::other(e).into())
}

pub fn apply_hotkeys(
    app: &AppHandle,
    search: &str,
    clipboard: &str,
    feature_hotkeys_json: &str,
    keywords_json: &str,
) -> Result<(), String> {
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
    // Chỉ đăng ký ĐÚNG phím clipboard người dùng đặt — không có phím dự phòng.
    // (Win+V dùng low-level hook riêng, không cần register global shortcut.)
    if let Some(sc) = shortcut_for(clipboard) {
        gs.register(sc).map_err(|e| format!("Hotkey clipboard đang bị chiếm: {e}"))?;
    }
    let feature_hotkeys: HashMap<String, String> = serde_json::from_str(feature_hotkeys_json).unwrap_or_default();
    let keyword_overrides: HashMap<String, String> = serde_json::from_str(keywords_json).unwrap_or_default();
    let mut bindings = Vec::new();
    for (id, hotkey) in feature_hotkeys {
        let hotkey = hotkey.trim().to_string();
        if hotkey.is_empty() { continue; }
        let shortcut = shortcut_for(&hotkey).ok_or_else(|| format!("Hotkey tính năng {id} không hợp lệ: {hotkey}"))?;
        gs.register(shortcut).map_err(|e| format!("Hotkey {hotkey} của {id} đang bị chiếm hoặc bị trùng: {e}"))?;
        let keyword = keyword_overrides.get(&id).cloned().filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| default_feature_keyword(&id).to_string());
        bindings.push(FeatureBinding { id, hotkey, keyword });
    }
    WIN_V_ENABLED.store(clipboard == "Win+V", Ordering::Relaxed);
    if let Ok(mut c) = config().write() { *c = (search.to_string(), clipboard.to_string()); }
    if let Ok(mut items) = feature_bindings().write() { *items = bindings; }
    Ok(())
}

fn default_feature_keyword(id: &str) -> &'static str {
    match id {
        "fulltext" => "in", "translate" => "tr", "wiki" => "wiki", "review" => "review",
        "formula" => "formula", "chemistry" => "chem", "google" => "g", "youtube" => "yt",
        "ocr" => "ocr", "convert" => "conv", "time" => "time", "url" => "url",
        "password" => "pw", "generator" => "#", "snippet" => ";", "process" => "ps",
        "system" => "sys", "window" => "<", "vscode" => "{", "service" => "!",
        "registry" => ":", "terminal" => ">",
        "port" => "port", "json" => "json", "jwt" => "jwt", "latex" => "latex", _ => "",
    }
}

fn activate_feature_hotkey(app: AppHandle, binding: FeatureBinding) {
    // Đợi người dùng nhả tổ hợp Alt/Ctrl/Shift rồi mới gửi Ctrl+C.
    std::thread::sleep(std::time::Duration::from_millis(120));
    let selected = if binding.id == "ocr" { None } else { capture_selected_text() };
    let prefill = if let Some(text) = selected.filter(|s| !s.trim().is_empty()) {
        format!("{} {}", binding.keyword, text.trim())
    } else if matches!(binding.id.as_str(), "ocr" | "review") {
        binding.keyword
    } else {
        format!("{} ", binding.keyword)
    };
    crate::core::window::toggle_with(&app, "search", &prefill);
}

fn capture_selected_text() -> Option<String> {
    crate::commands::clipboard::suppress_watcher_for(2_500);
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let saved_text = clipboard.get_text().ok();
    let saved_image = if saved_text.is_none() {
        clipboard.get_image().ok().map(|img| (img.width, img.height, img.bytes.into_owned()))
    } else { None };
    let sentinel = format!("__HEASPOT_SELECTION_{}__", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos());
    clipboard.set_text(sentinel.clone()).ok()?;

    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP};
        const VK_CONTROL: u8 = 0x11;
        const VK_C: u8 = 0x43;
        keybd_event(VK_CONTROL, 0, 0, 0);
        keybd_event(VK_C, 0, 0, 0);
        keybd_event(VK_C, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0);
    }

    let mut selected = None;
    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(35));
        if let Ok(text) = clipboard.get_text() {
            if text != sentinel {
                selected = (!text.trim().is_empty() && text.len() <= 20_000).then_some(text);
                break;
            }
        }
    }

    if let Some(text) = saved_text {
        let _ = clipboard.set_text(text);
    } else if let Some((width, height, bytes)) = saved_image {
        let _ = clipboard.set_image(arboard::ImageData {
            width, height, bytes: std::borrow::Cow::Owned(bytes),
        });
    } else {
        let _ = clipboard.set_text(String::new());
    }
    selected
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

#[cfg(test)]
mod tests {
    #[test]
    fn parses_user_feature_hotkey() {
        assert!(super::shortcut_for("Alt+Shift+T").is_some());
        assert!(super::shortcut_for("Ctrl+Alt+F").is_some());
    }
}
