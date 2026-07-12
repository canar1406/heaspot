use std::process::Command;

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

/// Mở file / thư mục / app (.lnk, .exe...) bằng shell mặc định.
/// App UWP/Store (`shell:AppsFolder\<AppID>`) phải mở qua explorer.exe.
#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
    if path.starts_with("shell:AppsFolder\\") {
        return std::process::Command::new("explorer.exe")
            .arg(&path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string());
    }
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
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
        return Err(format!("không chạy được với quyền admin (mã {})", r as isize));
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

/// Chạy uninstall entry mà Control Panel sử dụng; fallback mở Programs and Features.
#[tauri::command]
pub fn uninstall_app(title: String, path: String) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if let Some((display, mut command)) = find_uninstall_entry(&title, &path) {
        let lower = command.to_lowercase();
        if lower.contains("msiexec") {
            if let Some(pos) = lower.find("/i") { command.replace_range(pos..pos + 2, "/X"); }
        }
        Command::new("cmd").args(["/C", &command]).creation_flags(CREATE_NO_WINDOW)
            .spawn().map_err(|e| e.to_string())?;
        return Ok(format!("Đã mở trình gỡ cài đặt của {display}"));
    }
    Command::new("control.exe").arg("appwiz.cpl").spawn().map_err(|e| e.to_string())?;
    Ok("Không xác định được gói chính xác; đã mở Programs and Features".into())
}

fn find_uninstall_entry(title: &str, app_path: &str) -> Option<(String, String)> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let needle = normalize_app_name(title);
    let target = app_path.trim_matches('"').to_lowercase();
    let locations = [
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];
    let mut best: Option<(i32, String, String)> = None;
    for (root, location) in locations {
        let Ok(key) = RegKey::predef(root).open_subkey(location) else { continue };
        for child in key.enum_keys().flatten() {
            let Ok(entry) = key.open_subkey(child) else { continue };
            let display: String = entry.get_value("DisplayName").unwrap_or_default();
            let uninstall: String = entry.get_value("UninstallString").unwrap_or_default();
            if display.is_empty() || uninstall.is_empty() { continue; }
            let d = normalize_app_name(&display);
            let icon: String = entry.get_value("DisplayIcon").unwrap_or_default();
            let install: String = entry.get_value("InstallLocation").unwrap_or_default();
            let mut score = 0;
            if d == needle { score += 100; }
            else if d.contains(&needle) || needle.contains(&d) { score += 65; }
            let icon_path = icon.trim_matches('"').split(',').next().unwrap_or("").to_lowercase();
            if !icon_path.is_empty() && target.contains(&icon_path) { score += 50; }
            if !install.is_empty() && target.starts_with(&install.to_lowercase()) { score += 40; }
            if score > best.as_ref().map(|x| x.0).unwrap_or(20) { best = Some((score, display, uninstall)); }
        }
    }
    best.map(|(_, display, command)| (display, command))
}

fn normalize_app_name(value: &str) -> String {
    value.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Start / Stop / Restart một Windows Service (chạy PowerShell elevated)
#[tauri::command]
pub fn service_action(name: String, action: String) -> Result<(), String> {
    // Chỉ cho phép tên service hợp lệ, chặn injection
    if !name.chars().all(|c| c.is_alphanumeric() || "._- ".contains(c)) {
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
