use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use walkdir::WalkDir;

#[derive(Serialize, Clone, Debug)]
pub struct AppEntry {
    pub name: String,
    pub path: String,
    /// Icon thật của app dạng data URL (PNG base64), None nếu không lấy được
    pub icon: Option<String>,
}

/// Cache icon theo path — extraction chỉ tốn công một lần cho mỗi app
static ICON_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

pub(crate) fn icon_for(path: &str) -> Option<String> {
    let cache = ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().ok()?;
    if let Some(v) = map.get(path) {
        return v.clone();
    }
    let icon = crate::core::icons::extract_icon_data_url(path);
    map.insert(path.to_string(), icon.clone());
    icon
}

#[derive(Serialize, Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

/// Worker chạy nền: quét app + file định kỳ 10 phút, cache vào AppState
pub fn spawn_index_workers(app: AppHandle) {
    std::thread::spawn(move || {
        // COM cần cho SHGetFileInfoW resolve icon của .lnk
        unsafe {
            use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
            CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as _);
        }
        loop {
            let apps = scan_apps();
            let files = scan_files();
            {
                let state = app.state::<crate::AppState>();
                if let Ok(mut w) = state.apps.write() {
                    *w = apps;
                }
                if let Ok(mut w) = state.files.write() {
                    *w = files;
                };
            }
            std::thread::sleep(Duration::from_secs(600));
        }
    });
}

/// Quét Start Menu (system + user) và Registry App Paths
pub fn scan_apps() -> Vec<AppEntry> {
    let mut apps: Vec<AppEntry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let mut start_menu_dirs: Vec<PathBuf> = vec![PathBuf::from(
        r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs",
    )];
    if let Some(roaming) = dirs::config_dir() {
        start_menu_dirs.push(roaming.join(r"Microsoft\Windows\Start Menu\Programs"));
    }

    for dir in start_menu_dirs {
        for entry in WalkDir::new(&dir)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext != "lnk" && ext != "url" && ext != "exe" {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            // Bỏ các shortcut rác thường gặp
            let lower = name.to_lowercase();
            if lower.contains("uninstall") || lower.contains("gỡ cài đặt") {
                continue;
            }
            if seen.insert(lower) {
                let path_str = path.to_string_lossy().to_string();
                apps.push(AppEntry {
                    name: name.to_string(),
                    icon: icon_for(&path_str),
                    path: path_str,
                });
            }
        }
    }

    // Registry: HKLM\...\App Paths (các exe đã đăng ký)
    scan_app_paths_registry(&mut apps, &mut seen);

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

fn scan_app_paths_registry(apps: &mut Vec<AppEntry>, seen: &mut HashSet<String>) {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let Ok(app_paths) =
        hklm.open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths")
    else {
        return;
    };
    for key in app_paths.enum_keys().flatten() {
        let Ok(sub) = app_paths.open_subkey(&key) else {
            continue;
        };
        let Ok(exe_path) = sub.get_value::<String, _>("") else {
            continue;
        };
        let exe_path = exe_path.trim_matches('"').to_string();
        if !Path::new(&exe_path).exists() {
            continue;
        }
        let name = key.trim_end_matches(".exe").to_string();
        if name.is_empty() {
            continue;
        }
        if seen.insert(name.to_lowercase()) {
            apps.push(AppEntry {
                name,
                icon: icon_for(&exe_path),
                path: exe_path,
            });
        }
    }
}

/// Quét file trong các thư mục người dùng hay đụng tới (fallback khi không có Everything)
pub fn scan_files() -> Vec<FileEntry> {
    const MAX_ENTRIES: usize = 60_000;
    let mut files: Vec<FileEntry> = Vec::new();

    let roots: Vec<Option<PathBuf>> = vec![
        dirs::desktop_dir(),
        dirs::document_dir(),
        dirs::download_dir(),
        dirs::picture_dir(),
        dirs::video_dir(),
        dirs::audio_dir(),
    ];

    for root in roots.into_iter().flatten() {
        for entry in WalkDir::new(&root)
            .max_depth(5)
            .into_iter()
            .filter_entry(|e| !is_ignored(e.path()))
            .filter_map(|e| e.ok())
        {
            if files.len() >= MAX_ENTRIES {
                return files;
            }
            let path = entry.path();
            if path == root {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            files.push(FileEntry {
                name: name.to_string(),
                path: path.to_string_lossy().to_string(),
                is_dir: path.is_dir(),
            });
        }
    }
    files
}

fn is_ignored(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    name.starts_with('.')
        || name.eq_ignore_ascii_case("node_modules")
        || name.eq_ignore_ascii_case("target")
        || name.eq_ignore_ascii_case("__pycache__")
        || name.eq_ignore_ascii_case("AppData")
}
