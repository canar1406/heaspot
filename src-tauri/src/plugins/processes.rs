//! Task Manager (`ps`): liệt kê tiến trình đang chạy + KILL mạnh.
//!
//! Kill "mạnh hơn Task Manager": bật SeDebugPrivilege rồi TerminateProcess;
//! nếu vẫn bị từ chối (process khác quyền/khác user), fallback sang
//! `taskkill /F /T` chạy dưới quyền Administrator (UAC) — kill cả cây con.

use serde::Serialize;

#[derive(Serialize)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub exe: String,
    pub mem_mb: f64,
    pub icon: Option<String>,
}

/// Liệt kê tiến trình (lọc theo tên/pid), sắp theo RAM giảm dần.
#[tauri::command]
pub fn list_processes(query: String) -> Vec<ProcInfo> {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let q = query.trim().to_lowercase();
    let mut out: Vec<ProcInfo> = Vec::new();

    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap.is_null() {
            return out;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut entry);
        while ok != 0 {
            let pid = entry.th32ProcessID;
            if pid != 0 {
                let name = {
                    let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(0);
                    String::from_utf16_lossy(&entry.szExeFile[..len])
                };
                let name_l = name.to_lowercase();
                if q.is_empty() || name_l.contains(&q) || pid.to_string().contains(&q) {
                    let exe = crate::plugins::window_walker::process_path_of(pid).unwrap_or_default();
                    out.push(ProcInfo {
                        pid,
                        mem_mb: process_mem_mb(pid),
                        icon: if exe.is_empty() { None } else { crate::core::indexer::icon_for(&exe) },
                        exe,
                        name,
                    });
                }
            }
            ok = Process32NextW(snap, &mut entry);
        }
        windows_sys::Win32::Foundation::CloseHandle(snap);
    }

    out.sort_by(|a, b| b.mem_mb.partial_cmp(&a.mem_mb).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(160);
    out
}

/// Dev: tìm tiến trình đang chiếm một cổng TCP (parse `netstat -ano`).
/// Trả về ProcInfo để tái dùng UI process + context menu Kill.
#[tauri::command]
pub fn list_port(port: String) -> Vec<ProcInfo> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let want = port.trim();
    let out = std::process::Command::new("netstat")
        .args(["-ano", "-p", "tcp"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let Ok(out) = out else { return Vec::new() };
    let text = String::from_utf8_lossy(&out.stdout);

    let mut seen = std::collections::HashSet::new();
    let mut pids: Vec<(u32, String)> = Vec::new(); // (pid, local_addr:state)
    for line in text.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        // TCP  local  foreign  STATE  PID
        if cols.len() < 5 || !cols[0].eq_ignore_ascii_case("TCP") {
            continue;
        }
        let local = cols[1];
        let local_port = local.rsplit(':').next().unwrap_or("");
        // Lọc theo cổng: rỗng = tất cả LISTENING; có số = đúng cổng đó
        let matches = if want.is_empty() {
            cols[3].eq_ignore_ascii_case("LISTENING")
        } else {
            local_port == want
        };
        if !matches {
            continue;
        }
        let Ok(pid) = cols[4].parse::<u32>() else { continue };
        if pid == 0 || !seen.insert(pid) {
            continue;
        }
        pids.push((pid, format!("{} · {}", local, cols[3])));
    }

    pids.into_iter()
        .take(30)
        .map(|(pid, addr)| {
            let exe = crate::plugins::window_walker::process_path_of(pid).unwrap_or_default();
            let name = std::path::Path::new(&exe)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("PID {pid}"));
            ProcInfo {
                pid,
                mem_mb: process_mem_mb(pid),
                icon: if exe.is_empty() { None } else { crate::core::indexer::icon_for(&exe) },
                exe,
                name: format!("{name}  ·  {addr}"),
            }
        })
        .collect()
}

fn process_mem_mb(pid: u32) -> f64 {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return 0.0;
        }
        let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let ok = K32GetProcessMemoryInfo(h, &mut pmc, pmc.cb);
        CloseHandle(h);
        if ok != 0 {
            (pmc.WorkingSetSize as f64) / 1_048_576.0
        } else {
            0.0
        }
    }
}

/// Bật đặc quyền SeDebugPrivilege cho tiến trình WinSpot (nếu chạy elevated).
unsafe fn enable_debug_privilege() {
    use windows_sys::Win32::Foundation::{CloseHandle, LUID};
    use windows_sys::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = std::ptr::null_mut();
    if OpenProcessToken(
        GetCurrentProcess(),
        TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
        &mut token,
    ) == 0
    {
        return;
    }
    let name: Vec<u16> = "SeDebugPrivilege".encode_utf16().chain(std::iter::once(0)).collect();
    let mut luid: LUID = std::mem::zeroed();
    if LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut luid) != 0 {
        let tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        AdjustTokenPrivileges(token, 0, &tp, 0, std::ptr::null_mut(), std::ptr::null_mut());
    }
    CloseHandle(token);
}

/// Kết thúc tiến trình. `tree` = kill cả tiến trình con.
#[tauri::command]
pub fn kill_process(pid: u32, tree: Option<bool>) -> Result<(), String> {
    let tree = tree.unwrap_or(false);

    // 1. Thử TerminateProcess trực tiếp (nhanh, không cần UAC) — nếu không kill cây.
    if !tree && unsafe { terminate_direct(pid) } {
        return Ok(());
    }

    // 2. taskkill thường (kill được cây con nếu /T)
    if run_taskkill(pid, tree, false) {
        return Ok(());
    }

    // 3. Fallback: taskkill dưới quyền Administrator (UAC) — mạnh nhất ở userland.
    crate::commands::system::shell_execute_runas(
        "taskkill.exe",
        Some(&format!("/F {} /PID {pid}", if tree { "/T" } else { "" })),
    )
    .map_err(|_| "không thể kết thúc tiến trình này (có thể là tiến trình được bảo vệ của hệ thống)".to_string())
}

unsafe fn terminate_direct(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    enable_debug_privilege();
    let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
    if h.is_null() {
        return false;
    }
    let ok = TerminateProcess(h, 1) != 0;
    CloseHandle(h);
    ok
}

fn run_taskkill(pid: u32, tree: bool, _elevated: bool) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut args: Vec<String> = vec!["/F".into(), "/PID".into(), pid.to_string()];
    if tree {
        args.push("/T".into());
    }
    std::process::Command::new("taskkill")
        .args(&args)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
