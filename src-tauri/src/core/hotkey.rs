use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::sync::RwLock;
use std::{collections::HashMap, str::FromStr};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Payload gửi lên UI khi đang GHI hotkey (hook nuốt phím rồi báo phím nào được nhấn).
#[derive(Clone, serde::Serialize)]
struct CapturePayload {
    key: String,
    is_mod: bool,
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
}

/// Alt+Space bị app khác chiếm (VD: PowerToys Run) -> chuyển sang bắt bằng LL hook
static ALT_SPACE_VIA_HOOK: AtomicBool = AtomicBool::new(false);
static WIN_V_ENABLED: AtomicBool = AtomicBool::new(true);
/// Launcher bind vào PHÍM WIN ĐƠN (thay Start menu) — bắt qua low-level hook.
static WIN_LAUNCHER_ENABLED: AtomicBool = AtomicBool::new(false);
/// Trạng thái theo dõi nhấn/nhả phím Win để phân biệt "gõ Win đơn" với "Win+X".
static WIN_KEY_DOWN: AtomicBool = AtomicBool::new(false);
static WIN_OTHER_KEY: AtomicBool = AtomicBool::new(false);
/// UI đang GHI hotkey: hook nuốt MỌI phím (kể cả Win+E, Alt+Space của Windows)
/// và gửi phím đó về UI -> "focus tuyệt đối", không hotkey/OS shortcut nào chạy.
static CAPTURE_MODE: AtomicBool = AtomicBool::new(false);
/// Trạng thái modifier TỰ theo dõi khi ghi (GetAsyncKeyState không phản ánh phím đã nuốt).
static CAP_WIN: AtomicBool = AtomicBool::new(false);
static CAP_CTRL: AtomicBool = AtomicBool::new(false);
static CAP_ALT: AtomicBool = AtomicBool::new(false);
static CAP_SHIFT: AtomicBool = AtomicBool::new(false);

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
    // "Win+V" và "Win" (đơn) bắt bằng low-level hook, không đăng ký global shortcut.
    if value.eq_ignore_ascii_case("Win+V") || value.eq_ignore_ascii_case("Win") { return None; }
    // HotkeyCapture (UI) xuất "Win" cho phím Windows; parser global-shortcut cần "Super".
    let normalized = if value.len() >= 4 && value[..4].eq_ignore_ascii_case("Win+") {
        format!("Super+{}", &value[4..])
    } else {
        value.to_string()
    };
    Shortcut::from_str(&normalized).ok()
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

/// Tạm ngưng MỌI hotkey (global shortcut + low-level hook) trong lúc UI đang
/// ghi hotkey mới — nếu không, nhấn Alt+Space/Win+V… sẽ kích hoạt hành động
/// thay vì được ô ghi lại.
#[tauri::command]
pub fn suspend_hotkeys(app: AppHandle) -> Result<(), String> {
    // Bật CAPTURE_MODE: hook sẽ nuốt mọi phím khi cửa sổ app đang focus.
    CAPTURE_MODE.store(true, Ordering::Relaxed);
    CAP_WIN.store(false, Ordering::Relaxed);
    CAP_CTRL.store(false, Ordering::Relaxed);
    CAP_ALT.store(false, Ordering::Relaxed);
    CAP_SHIFT.store(false, Ordering::Relaxed);
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;
    WIN_V_ENABLED.store(false, Ordering::Relaxed);
    ALT_SPACE_VIA_HOOK.store(false, Ordering::Relaxed);
    WIN_LAUNCHER_ENABLED.store(false, Ordering::Relaxed);
    Ok(())
}

/// Khôi phục hotkey từ cấu hình đã lưu (gọi khi UI ghi xong / rời ô).
#[tauri::command]
pub fn resume_hotkeys(app: AppHandle) -> Result<(), String> {
    CAPTURE_MODE.store(false, Ordering::Relaxed);
    register_shortcuts(&app).map_err(|e| e.to_string())
}

pub fn apply_hotkeys(
    app: &AppHandle,
    search: &str,
    clipboard: &str,
    feature_hotkeys_json: &str,
    keywords_json: &str,
) -> Result<(), String> {
    // Launcher có thể bind vào PHÍM WIN ĐƠN (thay Start menu) — bắt bằng hook, không
    // đăng ký global shortcut. Ngược lại phải là tổ hợp/phím hợp lệ.
    let search_is_win = search.eq_ignore_ascii_case("Win");
    let search_shortcut = if search_is_win {
        None
    } else {
        Some(shortcut_for(search).ok_or("Hotkey launcher không hợp lệ")?)
    };
    if clipboard != "Win+V" && shortcut_for(clipboard).is_none() { return Err("Hotkey clipboard không hợp lệ".into()); }
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| e.to_string())?;
    if let Some(sc) = search_shortcut {
        if let Err(e) = gs.register(sc) {
            if search == "Alt+Space" {
                ALT_SPACE_VIA_HOOK.store(true, Ordering::Relaxed);
            } else { return Err(format!("Hotkey launcher đang bị ứng dụng khác chiếm: {e}")); }
        } else {
            ALT_SPACE_VIA_HOOK.store(false, Ordering::Relaxed);
        }
    } else {
        ALT_SPACE_VIA_HOOK.store(false, Ordering::Relaxed);
    }
    WIN_LAUNCHER_ENABLED.store(search_is_win, Ordering::Relaxed);
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
        "password" => "pw", "otp" => "otp", "generator" => "#", "snippet" => ";", "process" => "ps",
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
    } else if matches!(binding.id.as_str(), "ocr" | "review" | "otp") {
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

/// Foreground window có thuộc process của app không (failsafe: chỉ nuốt phím khi
/// cửa sổ app đang focus, tránh khoá cứng bàn phím toàn hệ thống).
unsafe fn foreground_is_ours() -> bool {
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };
    let hwnd = GetForegroundWindow();
    if hwnd.is_null() {
        return false;
    }
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    pid == GetCurrentProcessId()
}

/// vkCode -> (tên phím, có phải modifier không) để GHI hotkey.
fn vk_to_key(vk: u32) -> (String, bool) {
    match vk {
        0x5B | 0x5C => return ("Win".into(), true),
        0x11 | 0xA2 | 0xA3 => return ("Ctrl".into(), true),
        0x12 | 0xA4 | 0xA5 => return ("Alt".into(), true),
        0x10 | 0xA0 | 0xA1 => return ("Shift".into(), true),
        _ => {}
    }
    let name = match vk {
        0x41..=0x5A | 0x30..=0x39 => ((vk as u8) as char).to_string(), // A-Z, 0-9
        0x60..=0x69 => (vk - 0x60).to_string(),                        // Numpad 0-9
        0x70..=0x87 => format!("F{}", vk - 0x6F),                      // F1-F24
        0x20 => "Space".into(),
        0x0D => "Enter".into(),
        0x09 => "Tab".into(),
        0x1B => "Escape".into(),
        0x08 => "Backspace".into(),
        0x2E => "Delete".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        0x2D => "Insert".into(),
        0xBA => ";".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xC0 => "`".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "'".into(),
        _ => return (String::new(), false),
    };
    (name, false)
}

unsafe extern "system" fn winv_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, GetAsyncKeyState, KEYEVENTF_KEYUP, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN,
        VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP,
    };

    const VK_V: u32 = 0x56;
    const VK_SPACE_CODE: u32 = 0x20;

    if code == HC_ACTION as i32 {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        let msg = wparam as u32;
        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
        let held = |vk: u16| (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0;

        // === CHẾ ĐỘ GHI HOTKEY: nuốt MỌI phím (kể cả Win+E, Alt+Space của Windows)
        // và báo phím vừa nhấn lên UI. Chỉ chạy khi cửa sổ app đang focus (failsafe). ===
        if CAPTURE_MODE.load(Ordering::Relaxed) && foreground_is_ours() {
            let vk = kb.vkCode;
            // TỰ theo dõi modifier: phím đã nuốt không cập nhật GetAsyncKeyState.
            match vk {
                0x5B | 0x5C => CAP_WIN.store(is_down, Ordering::Relaxed),
                0x11 | 0xA2 | 0xA3 => CAP_CTRL.store(is_down, Ordering::Relaxed),
                0x12 | 0xA4 | 0xA5 => CAP_ALT.store(is_down, Ordering::Relaxed),
                0x10 | 0xA0 | 0xA1 => CAP_SHIFT.store(is_down, Ordering::Relaxed),
                _ => {}
            }
            if is_down {
                let (key, is_mod) = vk_to_key(vk);
                if !key.is_empty() {
                    if let Some(app) = APP.get() {
                        let _ = app.emit(
                            "hotkey://capture",
                            CapturePayload {
                                key,
                                is_mod,
                                ctrl: CAP_CTRL.load(Ordering::Relaxed),
                                alt: CAP_ALT.load(Ordering::Relaxed),
                                shift: CAP_SHIFT.load(Ordering::Relaxed),
                                win: CAP_WIN.load(Ordering::Relaxed),
                            },
                        );
                    }
                }
            }
            return 1; // nuốt cả down/up -> focus tuyệt đối khi ghi
        }

        // === Phím WIN ĐƠN -> Launcher (thay Start menu, kiểu PowerToys) ===
        // Cho Win-down/up đi qua để Win+X vẫn chạy; chỉ khi "gõ Win rồi nhả mà
        // KHÔNG bấm phím nào khác" thì mở launcher + chèn 0xFF để chặn Start menu.
        if WIN_LAUNCHER_ENABLED.load(Ordering::Relaxed) {
            let is_win = kb.vkCode == VK_LWIN as u32 || kb.vkCode == VK_RWIN as u32;
            if is_win {
                if is_down {
                    WIN_KEY_DOWN.store(true, Ordering::Relaxed);
                    WIN_OTHER_KEY.store(false, Ordering::Relaxed);
                } else if is_up {
                    let lone = WIN_KEY_DOWN.load(Ordering::Relaxed)
                        && !WIN_OTHER_KEY.load(Ordering::Relaxed);
                    WIN_KEY_DOWN.store(false, Ordering::Relaxed);
                    if lone {
                        // Win còn "đang giữ" -> chèn 0xFF để Windows coi là Win+X -> KHÔNG mở Start
                        keybd_event(0xFF, 0, 0, 0);
                        keybd_event(0xFF, 0, KEYEVENTF_KEYUP, 0);
                        if let Some(app) = APP.get() {
                            crate::core::window::toggle(app, "search");
                        }
                    }
                }
            } else if is_down && WIN_KEY_DOWN.load(Ordering::Relaxed) {
                // Phím khác nhấn khi Win đang giữ -> Win+X, không phải gõ Win đơn
                WIN_OTHER_KEY.store(true, Ordering::Relaxed);
            }
        }

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
