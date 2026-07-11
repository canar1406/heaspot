//! Window Walker (`<`): liệt kê cửa sổ đang mở và chuyển focus — thay Alt+Tab.

use serde::Serialize;
use std::ffi::c_void;

#[derive(Serialize)]
pub struct OpenWindow {
    pub hwnd: isize,
    pub title: String,
    pub process: String,
    pub icon: Option<String>,
}

struct EnumState {
    windows: Vec<OpenWindow>,
    own_pid: u32,
}

#[tauri::command]
pub fn list_windows(query: String) -> Vec<OpenWindow> {
    use windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows;

    let mut state = EnumState {
        windows: Vec::new(),
        own_pid: std::process::id(),
    };
    unsafe {
        EnumWindows(Some(enum_proc), &mut state as *mut EnumState as isize);
    }

    let q = query.trim().to_lowercase();
    let mut list: Vec<OpenWindow> = state
        .windows
        .into_iter()
        .filter(|w| {
            q.is_empty()
                || w.title.to_lowercase().contains(&q)
                || w.process.to_lowercase().contains(&q)
        })
        .collect();
    list.truncate(15);
    list
}

unsafe extern "system" fn enum_proc(hwnd: *mut c_void, lparam: isize) -> i32 {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
    };

    let state = &mut *(lparam as *mut EnumState);

    if IsWindowVisible(hwnd) == 0 {
        return 1;
    }
    // Bỏ tool window (tooltip, popup ẩn...)
    if (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32) & WS_EX_TOOLWINDOW != 0 {
        return 1;
    }
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return 1;
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    if got <= 0 {
        return 1;
    }
    let title = String::from_utf16_lossy(&buf[..got as usize]);

    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == state.own_pid {
        return 1;
    }
    let exe_path = process_path(pid).unwrap_or_default();
    let process = std::path::Path::new(&exe_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let icon = if exe_path.is_empty() {
        None
    } else {
        crate::core::indexer::icon_for(&exe_path)
    };

    state.windows.push(OpenWindow {
        hwnd: hwnd as isize,
        title,
        process,
        icon,
    });
    1
}

/// Public helper: đường dẫn exe của một PID (dùng chung cho clipboard source_app)
pub(crate) fn process_path_of(pid: u32) -> Option<String> {
    unsafe { process_path(pid) }
}

unsafe fn process_path(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if handle.is_null() {
        return None;
    }
    let mut buf = vec![0u16; 1024];
    let mut size = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
    CloseHandle(handle);
    (ok != 0).then(|| String::from_utf16_lossy(&buf[..size as usize]))
}

/// Chuyển focus sang cửa sổ được chọn (restore nếu đang minimize)
#[tauri::command]
pub fn focus_window(hwnd: isize) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    unsafe {
        let h = hwnd as *mut c_void;
        if IsIconic(h) != 0 {
            ShowWindow(h, SW_RESTORE);
        }
        SetForegroundWindow(h);
    }
    Ok(())
}
