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

/// Mở file / thư mục / app (.lnk, .exe...) bằng shell mặc định
#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
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

/// ShellExecuteW với verb "runas" — hiện UAC prompt
fn shell_execute_runas(file: &str, args: Option<&str>) -> Result<(), String> {
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
