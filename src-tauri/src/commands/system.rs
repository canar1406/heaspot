use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, serde::Serialize)]
pub struct UninstallProgress {
    operation_id: String,
    app_name: String,
    stage: String,
    detail: String,
    percent: Option<u8>,
    done: bool,
    success: bool,
}

static UNINSTALL_RUNNING: AtomicBool = AtomicBool::new(false);
static UNINSTALL_PROGRESS: OnceLock<Mutex<Option<UninstallProgress>>> = OnceLock::new();

fn uninstall_progress_state() -> &'static Mutex<Option<UninstallProgress>> {
    UNINSTALL_PROGRESS.get_or_init(|| Mutex::new(None))
}

fn publish_uninstall_progress(app: &AppHandle, progress: UninstallProgress) {
    if let Ok(mut current) = uninstall_progress_state().lock() {
        *current = Some(progress.clone());
    }
    let _ = app.emit("uninstall://progress", progress);
}

fn show_uninstall_progress(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("uninstall-progress") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn uninstall_is_running() -> bool {
    UNINSTALL_RUNNING.load(Ordering::Acquire)
}

pub fn show_current_uninstall(app: &AppHandle) {
    if uninstall_progress_state()
        .lock()
        .ok()
        .is_some_and(|progress| progress.is_some())
    {
        show_uninstall_progress(app);
    }
}

#[tauri::command]
pub fn get_uninstall_progress() -> Option<UninstallProgress> {
    uninstall_progress_state()
        .lock()
        .ok()
        .and_then(|progress| progress.clone())
}

/// Lệnh hệ thống: sleep, shutdown, restart, lock, empty-trash, mute
#[tauri::command]
pub fn system_command(action: String) -> Result<(), String> {
    match action.as_str() {
        "shutdown" => {
            spawn("shutdown", &["/s", "/t", "0"])?;
        }
        "restart" => {
            spawn("shutdown", &["/r", "/t", "0"])?;
        }
        "sleep" => unsafe {
            use windows_sys::Win32::System::Power::SetSuspendState;
            SetSuspendState(0, 0, 0);
        },
        "lock" => unsafe {
            use windows_sys::Win32::System::Shutdown::LockWorkStation;
            LockWorkStation();
        },
        "empty-trash" => unsafe {
            use windows_sys::Win32::UI::Shell::SHEmptyRecycleBinW;
            // 0x7 = không hỏi xác nhận + không progress + không âm thanh
            SHEmptyRecycleBinW(std::ptr::null_mut(), std::ptr::null(), 0x7);
        },
        "mute" => unsafe {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP};
            const VK_VOLUME_MUTE: u8 = 0xAD;
            keybd_event(VK_VOLUME_MUTE, 0, 0, 0);
            keybd_event(VK_VOLUME_MUTE, 0, KEYEVENTF_KEYUP, 0);
        },
        _ => return Err(format!("lệnh không hỗ trợ: {action}")),
    }
    Ok(())
}

fn spawn(cmd: &str, args: &[&str]) -> Result<(), String> {
    Command::new(cmd)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Mode Terminal (gõ `>` trước lệnh): mở cửa sổ cmd mới và chạy lệnh
#[tauri::command]
pub fn run_in_terminal(command: String) -> Result<(), String> {
    Command::new("cmd")
        .args(["/C", "start", "cmd", "/K", &command])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Mở file / thư mục / app (.lnk, .exe, UWP) bằng shell mặc định.
/// Dùng explorer.exe cho MỌI thứ: nó resolve .lnk, chạy .exe, mở file/folder và
/// app UWP (`shell:AppsFolder\...`) đáng tin cậy — không phụ thuộc COM apartment
/// của thread lệnh (ShellExecuteW/opener "báo thành công" nhưng không launch .lnk).
#[tauri::command]
pub fn open_path(path: String, state: tauri::State<'_, crate::AppState>) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Ok(conn) = state.db.lock() {
        let _ = conn.execute(
            "INSERT INTO launch_usage(path, launch_count, last_launched) VALUES (?1, 1, datetime('now', 'localtime'))
             ON CONFLICT(path) DO UPDATE SET launch_count = launch_count + 1, last_launched = datetime('now', 'localtime')",
            rusqlite::params![path],
        );
        let _ = conn.execute(
            "DELETE FROM launch_usage WHERE path NOT IN (SELECT path FROM launch_usage ORDER BY last_launched DESC LIMIT 1000)",
            [],
        );
    }
    Ok(())
}

/// Mở URL bằng trình duyệt mặc định (Web Search g/yt/wiki)
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Copy text vào clipboard (dùng cho Calculator & Snippets)
#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(text).map_err(|e| e.to_string())
}

/// Đọc text hiện có trong clipboard (dùng cho công cụ dev: json, jwt…)
#[tauri::command]
pub fn get_clipboard_text() -> Result<String, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.get_text().map_err(|e| e.to_string())
}

/// ShellExecuteW với verb "runas" — hiện UAC prompt
pub(crate) fn shell_execute_runas(file: &str, args: Option<&str>) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let to_wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let verb = to_wide("runas");
    let file_w = to_wide(file);
    let args_w = args.map(to_wide);
    let r = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file_w.as_ptr(),
            args_w.as_ref().map_or(std::ptr::null(), |a| a.as_ptr()),
            std::ptr::null(),
            1, // SW_SHOWNORMAL
        )
    };
    // ShellExecuteW trả về giá trị > 32 nếu thành công; 1223 = user bấm Cancel UAC
    if (r as isize) <= 32 {
        return Err(format!(
            "không chạy được với quyền admin (mã {})",
            r as isize
        ));
    }
    Ok(())
}

/// Context menu: chạy app/file với quyền Administrator
#[tauri::command]
pub fn run_as_admin(path: String) -> Result<(), String> {
    shell_execute_runas(&path, None)
}

/// Context menu: mở Explorer và select đúng file
#[tauri::command]
pub fn open_file_location(path: String) -> Result<(), String> {
    Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Chạy đúng uninstaller của app. Không fallback sang danh sách Control Panel
/// chung vì thao tác đó vừa chậm vừa bắt người dùng tìm lại ứng dụng.
#[tauri::command]
pub fn uninstall_app(
    app: AppHandle,
    operation_id: String,
    title: String,
    path: String,
) -> Result<String, String> {
    validate_uninstall_source(&path)?;
    if UNINSTALL_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        show_uninstall_progress(&app);
        return Err("Một tác vụ gỡ cài đặt khác đang chạy. Tiến trình hiện tại đã được mở.".into());
    }

    publish_uninstall_progress(
        &app,
        UninstallProgress {
            operation_id: operation_id.clone(),
            app_name: title.clone(),
            stage: "Đang chuẩn bị".into(),
            detail: "HeaSpot đang kiểm tra ứng dụng và tìm trình gỡ cài đặt phù hợp.".into(),
            percent: Some(5),
            done: false,
            success: false,
        },
    );
    show_uninstall_progress(&app);

    std::thread::spawn(move || run_uninstall(app, operation_id, title, path));
    Ok("Đã bắt đầu gỡ cài đặt trong cửa sổ tiến trình riêng".into())
}

fn validate_uninstall_source(path: &str) -> Result<(), String> {
    if path.starts_with("shell:AppsFolder\\") {
        return Ok(());
    }
    let source = std::path::PathBuf::from(path.trim_matches('"'));
    if !source.is_file() {
        return Err("Đường dẫn ứng dụng không còn tồn tại; HeaSpot đã dừng để tránh khớp nhầm và gỡ sai ứng dụng.".into());
    }
    let source = source
        .canonicalize()
        .map_err(|error| format!("Không xác minh được đường dẫn ứng dụng trước khi gỡ: {error}"))?;
    if std::env::current_exe()
        .ok()
        .and_then(|current| current.canonicalize().ok())
        .is_some_and(|current| current == source)
    {
        return Err("HeaSpot không tự gỡ chính tiến trình đang chạy. Hãy dùng Windows Installed apps để gỡ HeaSpot.".into());
    }
    Ok(())
}

fn run_uninstall(app: AppHandle, operation_id: String, title: String, path: String) {
    let report = |stage: &str, detail: String, percent: Option<u8>, done: bool, success: bool| {
        publish_uninstall_progress(
            &app,
            UninstallProgress {
                operation_id: operation_id.clone(),
                app_name: title.clone(),
                stage: stage.into(),
                detail,
                percent,
                done,
                success,
            },
        );
    };

    report(
        "Đang tìm trình gỡ cài đặt",
        "Đang đọc thông tin uninstall từ Registry 32-bit và 64-bit của Windows.".into(),
        Some(20),
        false,
        false,
    );

    if let Some(entry) = find_uninstall_entry(&title, &path) {
        let display = entry.display.clone();
        report(
            "Đã tìm thấy trình gỡ cài đặt",
            format!("Đang mở trình gỡ cài đặt chính thức của {display}."),
            Some(50),
            false,
            false,
        );
        match launch_uninstaller(&entry.command) {
            Ok(mut launched) => {
                report(
                    "Đang gỡ cài đặt",
                    "Đang chờ trình gỡ cài đặt của hãng hoàn tất. Phần trăm thực (nếu có) được hiển thị trong cửa sổ của hãng.".into(),
                    None,
                    false,
                    false,
                );
                match launched.child.wait() {
                    Ok(status) if status.success() || status.code() == Some(3010) => {
                        if !wait_for_vendor_uninstall(&entry, &launched.executable) {
                            report(
                                "Đã hủy hoặc chưa gỡ",
                                format!(
                                    "{display} vẫn còn đăng ký trong Windows. HeaSpot không quét/xóa phần còn sót vì chưa có bằng chứng uninstaller đã hoàn tất."
                                ),
                                Some(100),
                                true,
                                false,
                            );
                            UNINSTALL_RUNNING.store(false, Ordering::Release);
                            return;
                        }
                        report(
                            "Đang quét phần còn sót",
                            "Uninstaller đã kết thúc. HeaSpot đang quét file, thư mục, Registry và service còn lại.".into(),
                            Some(88),
                            false,
                            false,
                        );
                        let cleanup = cleanup_uninstall_leftovers(&title, &path, &entry);
                        if cleanup.failed.is_empty() {
                            report(
                                "Hoàn tất và đã quét sạch",
                                format!(
                                    "Đã gỡ {display} và dọn {} dấu vết còn sót có độ tin cậy cao.",
                                    cleanup.removed
                                ),
                                Some(100),
                                true,
                                true,
                            );
                        } else {
                            report(
                                "Hoàn tất nhưng còn mục không xóa được",
                                format!(
                                    "Đã dọn {} mục; còn {} mục bị khóa hoặc thiếu quyền: {}",
                                    cleanup.removed,
                                    cleanup.failed.len(),
                                    cleanup.failed.join("; ")
                                ),
                                Some(100),
                                true,
                                false,
                            );
                        }
                    }
                    Ok(status) => report(
                        "Không hoàn tất",
                        format!(
                            "Trình gỡ cài đặt đã đóng với mã {:?}. Ứng dụng có thể chưa được gỡ hoặc thao tác đã bị hủy.",
                            status.code()
                        ),
                        Some(100),
                        true,
                        false,
                    ),
                    Err(error) => report(
                        "Không theo dõi được tiến trình",
                        format!("Không chờ được trình gỡ cài đặt: {error}"),
                        Some(100),
                        true,
                        false,
                    ),
                }
                UNINSTALL_RUNNING.store(false, Ordering::Release);
                return;
            }
            Err(error) => {
                report(
                    "Không thể mở trình gỡ cài đặt",
                    error,
                    Some(100),
                    true,
                    false,
                );
                UNINSTALL_RUNNING.store(false, Ordering::Release);
                return;
            }
        }
    }
    if let Some(app_id) = path.strip_prefix("shell:AppsFolder\\") {
        let family = app_id.split('!').next().unwrap_or("");
        if !family.is_empty()
            && family
                .chars()
                .all(|c| c.is_alphanumeric() || "._-".contains(c))
        {
            report(
                "Đang gỡ ứng dụng Windows",
                "Windows đang xóa gói Appx/MSIX và dữ liệu đăng ký liên quan.".into(),
                Some(55),
                false,
                false,
            );
            let script = "$pkg = Get-AppxPackage | Where-Object { $_.PackageFamilyName -eq $env:HEASPOT_PACKAGE_FAMILY } | Select-Object -First 1; if (-not $pkg) { throw 'Package not found' }; Remove-AppxPackage -Package $pkg.PackageFullName";
            match crate::commands::run_hidden_ps(script, &[("HEASPOT_PACKAGE_FAMILY", family)]) {
                Ok(_) => report(
                    "Hoàn tất",
                    format!("Đã gỡ cài đặt {title}."),
                    Some(100),
                    true,
                    true,
                ),
                Err(error) => report(
                    "Không thể gỡ ứng dụng Windows",
                    error,
                    Some(100),
                    true,
                    false,
                ),
            }
            UNINSTALL_RUNNING.store(false, Ordering::Release);
            return;
        }
    }
    // Portable executable: there is no Windows uninstall registration. Allow
    // removing the exact file only when it lives inside the user's profile;
    // never treat Program Files/Windows or a directory as a portable target.
    let portable = std::path::PathBuf::from(path.trim_matches('"'));
    let current = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok());
    let candidate = portable.canonicalize().ok();
    let home = dirs::home_dir().and_then(|p| p.canonicalize().ok());
    if portable
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        && candidate.as_ref().is_some_and(|p| p.is_file())
        && home
            .as_ref()
            .zip(candidate.as_ref())
            .is_some_and(|(h, p)| p.starts_with(h))
        && candidate != current
    {
        report(
            "Đang xóa ứng dụng portable",
            "Ứng dụng không đăng ký uninstaller; HeaSpot chỉ xóa đúng file .exe đã chọn.".into(),
            Some(70),
            false,
            false,
        );
        match std::fs::remove_file(candidate.unwrap()) {
            Ok(()) => report(
                "Hoàn tất",
                format!("Đã xóa bản portable {title}."),
                Some(100),
                true,
                true,
            ),
            Err(error) => report(
                "Không thể xóa ứng dụng portable",
                format!("Không xóa được bản portable {title}: {error}"),
                Some(100),
                true,
                false,
            ),
        }
        UNINSTALL_RUNNING.store(false, Ordering::Release);
        return;
    }
    report(
        "Không tìm thấy trình gỡ cài đặt",
        format!(
            "Không tìm thấy trình gỡ cài đặt chính xác cho {title}. HeaSpot không mở danh sách Control Panel chung để tránh gỡ nhầm ứng dụng."
        ),
        Some(100),
        true,
        false,
    );
    UNINSTALL_RUNNING.store(false, Ordering::Release);
}

#[derive(Clone)]
struct UninstallEntry {
    display: String,
    command: String,
    install_location: String,
    display_icon: String,
    registry_root: &'static str,
    registry_path: String,
}

fn find_uninstall_entry(title: &str, app_path: &str) -> Option<UninstallEntry> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let needle = normalize_app_name(title);
    let target = app_path.trim_matches('"').to_lowercase();
    let locations = [
        (
            "HKCU",
            HKEY_CURRENT_USER,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            "HKCU",
            HKEY_CURRENT_USER,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            "HKLM",
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            "HKLM",
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];
    let mut best: Option<(i32, UninstallEntry)> = None;
    for (root_name, root, location) in locations {
        let Ok(key) = RegKey::predef(root).open_subkey(location) else {
            continue;
        };
        for child in key.enum_keys().flatten() {
            let Ok(entry) = key.open_subkey(&child) else {
                continue;
            };
            let display: String = entry.get_value("DisplayName").unwrap_or_default();
            let quiet: String = entry.get_value("QuietUninstallString").unwrap_or_default();
            let uninstall: String = entry.get_value("UninstallString").unwrap_or_default();
            if display.is_empty() || (uninstall.is_empty() && quiet.is_empty()) {
                continue;
            }
            let icon: String = entry.get_value("DisplayIcon").unwrap_or_default();
            let install: String = entry.get_value("InstallLocation").unwrap_or_default();
            let score = score_uninstall_match(&needle, title, &target, &display, &icon, &install);
            // Prefer the interactive vendor uninstaller. QuietUninstallString
            // is only a fallback; uninstalling silently from one menu click is
            // surprising and unlike Revo's confirmable workflow.
            let command = if uninstall.is_empty() {
                quiet
            } else {
                uninstall
            };
            if score >= 45 && score > best.as_ref().map(|x| x.0).unwrap_or(0) {
                best = Some((
                    score,
                    UninstallEntry {
                        display,
                        command,
                        install_location: install,
                        display_icon: icon,
                        registry_root: root_name,
                        registry_path: format!(r"{location}\{child}"),
                    },
                ));
            }
        }
    }
    best.map(|(_, entry)| entry)
}

fn normalize_app_name(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn normalize_windows_path(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

/// Component-aware containment for Registry/ImagePath strings. A raw prefix
/// would incorrectly treat `C:\\Vendor\\App2` as a child of
/// `C:\\Vendor\\App`, which is unsafe for uninstall matching and cleanup.
fn windows_path_is_within(path: &str, root: &str) -> bool {
    let path = normalize_windows_path(path);
    let root = normalize_windows_path(root);
    if path.is_empty() || root.is_empty() {
        return false;
    }
    path == root
        || path
            .strip_prefix(&root)
            .is_some_and(|suffix| suffix.starts_with('\\'))
}

fn app_name_tokens(value: &str) -> Vec<String> {
    const NOISE: &[&str] = &[
        "app",
        "application",
        "desktop",
        "launcher",
        "client",
        "file",
        "manager",
        "program",
        "software",
        "version",
        "setup",
        "installer",
        "uninstall",
        "x64",
        "x86",
        "64bit",
        "32bit",
        "trinh",
        "quan",
        "ly",
    ];
    let mut tokens: Vec<String> = value
        .to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .filter(|token| !NOISE.contains(token))
        // Version/build numbers are poor identity signals. Keep a single digit
        // because it is significant in product names such as 7-Zip.
        .filter(|token| token.len() == 1 || !token.chars().all(|ch| ch.is_ascii_digit()))
        .map(str::to_string)
        .collect();
    tokens.sort();
    tokens.dedup();
    tokens
}

fn score_uninstall_match(
    normalized_title: &str,
    title: &str,
    target: &str,
    display: &str,
    display_icon: &str,
    install_location: &str,
) -> i32 {
    let normalized_display = normalize_app_name(display);
    let mut score = if normalized_display == normalized_title {
        120
    } else if normalized_title.len() >= 4
        && normalized_display.len() >= 4
        && (normalized_display.contains(normalized_title)
            || normalized_title.contains(&normalized_display))
    {
        80
    } else {
        0
    };

    // Start-menu names often describe the executable ("7-Zip File Manager")
    // while Apps & Features describes the product/version ("7-Zip 24.xx
    // (x64)"). Compare stable product tokens and include shortcut parent names.
    let target_path = std::path::Path::new(target);
    let shortcut_name = target_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let shortcut_group = target_path
        .parent()
        .and_then(std::path::Path::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let title_context = format!("{title} {shortcut_name} {shortcut_group}");
    let title_tokens = app_name_tokens(&title_context);
    let display_tokens = app_name_tokens(display);
    let common: Vec<&String> = title_tokens
        .iter()
        .filter(|token| display_tokens.contains(token))
        .collect();
    if common.len() >= 2 {
        score += 70 + (common.len().min(4) as i32 - 2) * 8;
    } else if common.len() == 1 && common[0].len() >= 4 {
        score += 45;
    }

    let icon_path = display_icon
        .trim()
        .trim_matches('"')
        .split(',')
        .next()
        .unwrap_or("")
        .trim_matches('"')
        .replace('/', "\\")
        .to_lowercase();
    let install = install_location
        .trim()
        .trim_matches('"')
        .replace('/', "\\")
        .to_lowercase();
    let target = target.replace('/', "\\");
    if !icon_path.is_empty() && (target == icon_path || target.starts_with(&icon_path)) {
        score += 80;
    }
    if !install.is_empty() && windows_path_is_within(&target, &install) {
        score += 60;
    }
    score
}

/// Split a Windows command line while preserving quoted paths/arguments. For
/// unquoted registry strings, an executable path ending in `.exe` is accepted
/// even when it contains spaces (a common third-party installer mistake).
fn split_uninstall_command(command: &str) -> Vec<String> {
    let input = command.trim();
    if input.is_empty() {
        return Vec::new();
    }
    if !input.starts_with('"') {
        if let Some(end) = input.to_ascii_lowercase().find(".exe") {
            let exe_end = end + 4;
            let mut result = vec![input[..exe_end].trim().to_string()];
            result.extend(split_windows_args(input[exe_end..].trim()));
            return result;
        }
    }
    split_windows_args(input)
}

fn split_windows_args(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut args = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }
        let mut arg = String::new();
        let mut quoted = false;
        while index < chars.len() {
            match chars[index] {
                '"' => {
                    quoted = !quoted;
                    index += 1;
                }
                ch if ch.is_whitespace() && !quoted => break,
                ch => {
                    arg.push(ch);
                    index += 1;
                }
            }
        }
        if !arg.is_empty() {
            args.push(arg);
        }
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
    }
    args
}

fn expand_environment_variables(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        output.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            output.push_str(&rest[start..]);
            return output;
        };
        let name = &after[..end];
        if let Ok(replacement) = std::env::var(name) {
            output.push_str(&replacement);
        } else {
            output.push('%');
            output.push_str(name);
            output.push('%');
        }
        rest = &after[end + 1..];
    }
    output.push_str(rest);
    output
}

#[derive(Default)]
struct CleanupReport {
    removed: usize,
    failed: Vec<String>,
}

fn strong_product_name_match(candidate: &str, title: &str, display: &str) -> bool {
    let candidate_tokens = app_name_tokens(candidate);
    let reference_tokens = app_name_tokens(&format!("{title} {display}"));
    let common_tokens = candidate_tokens
        .iter()
        .filter(|token| reference_tokens.contains(token))
        .count();
    let candidate = normalize_app_name(candidate);
    if candidate.len() < 3 {
        return false;
    }
    let title = normalize_app_name(title);
    let display = normalize_app_name(display);
    common_tokens >= 2
        || candidate == title
        || candidate == display
        || (candidate.len() >= 5
            && ((title.starts_with(&candidate) && title.len() - candidate.len() <= 12)
                || (display.starts_with(&candidate) && display.len() - candidate.len() <= 12)))
}

fn safe_cleanup_path(path: &std::path::Path, title: &str, display: &str) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !strong_product_name_match(name, title, display) {
        return false;
    }
    let protected_names = [
        "windows",
        "users",
        "program files",
        "program files (x86)",
        "programdata",
        "appdata",
        "local",
        "roaming",
        "microsoft",
        "common files",
    ];
    !protected_names
        .iter()
        .any(|value| name.eq_ignore_ascii_case(value))
}

fn add_matching_children(
    candidates: &mut Vec<std::path::PathBuf>,
    root: Option<std::path::PathBuf>,
    title: &str,
    display: &str,
) {
    let Some(root) = root.filter(|root| root.is_dir()) else {
        return;
    };
    let Ok(children) = std::fs::read_dir(root) else {
        return;
    };
    for child in children.flatten() {
        let path = child.path();
        if safe_cleanup_path(&path, title, display) {
            candidates.push(path);
        }
    }
}

fn cleanup_uninstall_leftovers(
    title: &str,
    app_path: &str,
    entry: &UninstallEntry,
) -> CleanupReport {
    use std::collections::HashSet;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let mut report = CleanupReport::default();
    let mut candidates = Vec::new();
    let install_location = std::path::PathBuf::from(entry.install_location.trim_matches('"'));
    if install_location.exists() && safe_cleanup_path(&install_location, title, &entry.display) {
        candidates.push(install_location.clone());
    }

    let icon = entry
        .display_icon
        .trim()
        .trim_matches('"')
        .split(',')
        .next()
        .unwrap_or("")
        .trim_matches('"');
    if let Some(parent) = std::path::Path::new(icon).parent() {
        if parent.exists() && safe_cleanup_path(parent, title, &entry.display) {
            candidates.push(parent.to_path_buf());
        }
    }

    // Exact product-named roots used by most Win32 apps. Only immediate
    // children are considered; generic vendor/shared roots are never removed.
    add_matching_children(
        &mut candidates,
        dirs::data_local_dir(),
        title,
        &entry.display,
    );
    add_matching_children(&mut candidates, dirs::data_dir(), title, &entry.display);
    add_matching_children(
        &mut candidates,
        std::env::var_os("PROGRAMDATA").map(std::path::PathBuf::from),
        title,
        &entry.display,
    );

    let shortcut = std::path::PathBuf::from(app_path.trim_matches('"'));
    if shortcut
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
        && shortcut.exists()
        && strong_product_name_match(
            shortcut
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or(""),
            title,
            &entry.display,
        )
    {
        candidates.push(shortcut);
    }

    let current_exe = std::env::current_exe()
        .ok()
        .and_then(|path| path.canonicalize().ok());
    let mut seen = HashSet::new();
    candidates.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for path in candidates {
        let key = path.to_string_lossy().to_lowercase();
        if !seen.insert(key) {
            continue;
        }
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if current_exe
            .as_ref()
            .is_some_and(|current| current == &canonical || current.starts_with(&canonical))
        {
            continue;
        }
        let result = if canonical.is_dir() {
            std::fs::remove_dir_all(&canonical)
        } else {
            std::fs::remove_file(&canonical)
        };
        match result {
            Ok(()) => report.removed += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => report
                .failed
                .push(format!("{} ({error})", canonical.display())),
        }
    }

    // Remove only the exact uninstall registration captured before launch.
    // This is a deterministic leftover, not a fuzzy Registry search result.
    let root = if entry.registry_root == "HKCU" {
        RegKey::predef(HKEY_CURRENT_USER)
    } else {
        RegKey::predef(HKEY_LOCAL_MACHINE)
    };
    match root.delete_subkey_all(&entry.registry_path) {
        Ok(()) => report.removed += 1,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => report.failed.push(format!(
            "{}\\{} ({error})",
            entry.registry_root, entry.registry_path
        )),
    }

    // Services are high-confidence leftovers only when their executable path
    // is inside the captured install directory. Delete the service record;
    // never match by display name alone.
    if install_location.as_os_str().len() > 3 {
        let services =
            RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SYSTEM\CurrentControlSet\Services");
        if let Ok(services) = services {
            let install_key = install_location
                .to_string_lossy()
                .replace('/', "\\")
                .to_lowercase();
            for service_name in services.enum_keys().flatten() {
                let Ok(service) = services.open_subkey(&service_name) else {
                    continue;
                };
                let image: String = service.get_value("ImagePath").unwrap_or_default();
                let expanded = expand_environment_variables(&image)
                    .trim_matches('"')
                    .replace('/', "\\")
                    .to_lowercase();
                if !windows_path_is_within(&expanded, &install_key) {
                    continue;
                }
                match Command::new("sc.exe")
                    .args(["delete", &service_name])
                    .status()
                {
                    Ok(status) if status.success() => report.removed += 1,
                    Ok(status) => report
                        .failed
                        .push(format!("Service {service_name} (mã {:?})", status.code())),
                    Err(error) => report
                        .failed
                        .push(format!("Service {service_name} ({error})")),
                }
            }
        }
    }

    report
}

struct LaunchedUninstaller {
    child: Child,
    executable: std::path::PathBuf,
}

fn uninstall_registration_exists(entry: &UninstallEntry) -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let root = if entry.registry_root == "HKCU" {
        RegKey::predef(HKEY_CURRENT_USER)
    } else {
        RegKey::predef(HKEY_LOCAL_MACHINE)
    };
    root.open_subkey(&entry.registry_path).is_ok()
}

fn uninstall_process_is_running(executable: &std::path::Path, install_location: &str) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let executable = executable
        .to_string_lossy()
        .replace('/', "\\")
        .to_lowercase();
    let install = install_location
        .trim_matches('"')
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            if let Some(path) = crate::plugins::window_walker::process_path_of(entry.th32ProcessID)
            {
                let path = path.replace('/', "\\").to_lowercase();
                if path == executable
                    || (!install.is_empty() && windows_path_is_within(&path, &install))
                {
                    found = true;
                    break;
                }
            }
            ok = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        found
    }
}

fn wait_for_vendor_uninstall(entry: &UninstallEntry, executable: &std::path::Path) -> bool {
    let install = std::path::Path::new(entry.install_location.trim_matches('"'));
    let has_install_location = install.as_os_str().len() > 3;
    let mut no_process_samples = 0u8;
    // A launcher/stub may exit before its elevated UI appears. Give that child
    // time to start, then require Registry/install-path evidence before cleanup.
    for sample in 0..43_200u32 {
        let registered = uninstall_registration_exists(entry);
        let installed = !has_install_location || install.exists();
        let process_running = uninstall_process_is_running(executable, &entry.install_location);
        if vendor_uninstall_has_finished(registered, installed, process_running) {
            return true;
        }
        if sample < 6 || process_running {
            no_process_samples = 0;
        } else {
            no_process_samples = no_process_samples.saturating_add(1);
            if no_process_samples >= 8 {
                return false;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    false
}

fn vendor_uninstall_has_finished(
    registration_exists: bool,
    install_location_exists: bool,
    process_running: bool,
) -> bool {
    (!registration_exists || !install_location_exists) && !process_running
}

fn launch_uninstaller(command: &str) -> Result<LaunchedUninstaller, String> {
    let mut parts = split_uninstall_command(command);
    if parts.is_empty() {
        return Err("Registry chứa lệnh gỡ cài đặt rỗng".into());
    }
    let executable = expand_environment_variables(&parts.remove(0));
    if executable.to_ascii_lowercase().ends_with("msiexec.exe")
        || executable.eq_ignore_ascii_case("msiexec")
    {
        for arg in &mut parts {
            let lower = arg.to_ascii_lowercase();
            if lower == "/i" {
                *arg = "/X".into();
            } else if lower.starts_with("/i{") {
                arg.replace_range(..2, "/X");
            }
        }
    }
    match Command::new(&executable).args(&parts).spawn() {
        Ok(child) => Ok(LaunchedUninstaller {
            child,
            executable: std::path::PathBuf::from(&executable),
        }),
        Err(error) if error.raw_os_error() == Some(740) => {
            // ERROR_ELEVATION_REQUIRED: a fixed script plus environment
            // variables avoids interpolating Registry values into PowerShell.
            // `-Wait` also gives the progress window a process to monitor.
            let args_json = serde_json::to_string(&parts).map_err(|json_error| {
                format!("Không mã hóa được tham số uninstall: {json_error}")
            })?;
            let script = "$a = ConvertFrom-Json $env:HEASPOT_UNINSTALL_ARGS; $p = Start-Process -FilePath $env:HEASPOT_UNINSTALL_EXE -ArgumentList ([string[]]$a) -Verb RunAs -Wait -PassThru; exit $p.ExitCode";
            Command::new("powershell.exe")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", script])
                .env("HEASPOT_UNINSTALL_EXE", &executable)
                .env("HEASPOT_UNINSTALL_ARGS", args_json)
                .spawn()
                .map(|child| LaunchedUninstaller {
                    child,
                    executable: std::path::PathBuf::from(&executable),
                })
                .map_err(|elevated_error| {
                    format!("Không mở được trình gỡ cài đặt {executable} với quyền quản trị: {elevated_error}")
                })
        }
        Err(error) => Err(format!(
            "Không mở được trình gỡ cài đặt {executable}: {error}"
        )),
    }
}

#[cfg(test)]
mod uninstall_tests {
    use super::{
        normalize_app_name, safe_cleanup_path, score_uninstall_match, split_uninstall_command,
        strong_product_name_match, validate_uninstall_source, vendor_uninstall_has_finished,
        windows_path_is_within,
    };

    #[test]
    fn matches_7zip_start_menu_name_to_versioned_registry_name() {
        let title = "7-Zip File Manager";
        let score = score_uninstall_match(
            &normalize_app_name(title),
            title,
            r"c:\programdata\microsoft\windows\start menu\programs\7-zip\7-zip file manager.lnk",
            "7-Zip 24.09 (x64 edition)",
            r#""C:\Program Files\7-Zip\7zFM.exe""#,
            r"C:\Program Files\7-Zip\",
        );
        assert!(score >= 70, "unexpected match score: {score}");
    }

    #[test]
    fn splits_quoted_and_badly_unquoted_uninstall_commands() {
        assert_eq!(
            split_uninstall_command(r#""C:\Program Files\7-Zip\Uninstall.exe" /S"#),
            vec![r"C:\Program Files\7-Zip\Uninstall.exe", "/S"]
        );
        assert_eq!(
            split_uninstall_command(r"C:\Program Files\Vendor\uninstall.exe /remove /log"),
            vec![r"C:\Program Files\Vendor\uninstall.exe", "/remove", "/log"]
        );
    }

    #[test]
    fn cleanup_accepts_product_root_but_rejects_shared_system_roots() {
        assert!(safe_cleanup_path(
            std::path::Path::new(r"C:\Program Files\7-Zip"),
            "7-Zip File Manager",
            "7-Zip 24.09 (x64 edition)"
        ));
        assert!(!safe_cleanup_path(
            std::path::Path::new(r"C:\Program Files"),
            "Program Files",
            "Program Files"
        ));
        assert!(!safe_cleanup_path(
            std::path::Path::new(r"C:\Users\Me\AppData\Local\Microsoft"),
            "Microsoft Teams",
            "Microsoft Teams"
        ));
    }

    #[test]
    fn cleanup_name_matching_does_not_accept_unrelated_vendor_data() {
        assert!(strong_product_name_match(
            "7-Zip",
            "7-Zip File Manager",
            "7-Zip 24.09 (x64 edition)"
        ));
        assert!(!strong_product_name_match(
            "Vendor Shared",
            "Vendor Photo Editor",
            "Vendor Photo Editor 3"
        ));
    }

    #[test]
    fn uninstall_rejects_a_missing_source_before_registry_matching() {
        let error =
            validate_uninstall_source(r"C:\HeaSpot-Smoke-Does-Not-Exist\missing-application.exe")
                .expect_err("a missing path must never reach fuzzy Registry matching");
        assert!(error.contains("tránh khớp nhầm"));
    }

    #[test]
    fn cleanup_path_matching_requires_a_component_boundary() {
        assert!(windows_path_is_within(
            r"C:\Program Files\Vendor App\service.exe",
            r"C:\Program Files\Vendor App"
        ));
        assert!(!windows_path_is_within(
            r"C:\Program Files\Vendor App 2\service.exe",
            r"C:\Program Files\Vendor App"
        ));
    }

    #[test]
    fn registry_removal_does_not_prove_completion_while_vendor_process_runs() {
        assert!(!vendor_uninstall_has_finished(false, true, true));
        assert!(vendor_uninstall_has_finished(false, true, false));
        assert!(vendor_uninstall_has_finished(true, false, false));
        assert!(!vendor_uninstall_has_finished(true, true, false));
    }
}

/// Start / Stop / Restart một Windows Service (chạy PowerShell elevated)
#[tauri::command]
pub fn service_action(name: String, action: String) -> Result<(), String> {
    // Chỉ cho phép tên service hợp lệ, chặn injection
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || "._- ".contains(c))
    {
        return Err("tên service không hợp lệ".into());
    }
    let verb = match action.as_str() {
        "start" => "Start-Service",
        "stop" => "Stop-Service",
        "restart" => "Restart-Service",
        _ => return Err(format!("action không hỗ trợ: {action}")),
    };
    shell_execute_runas(
        "powershell.exe",
        Some(&format!(
            "-NoProfile -WindowStyle Hidden -Command \"{verb} -Name '{name}'\""
        )),
    )
}
