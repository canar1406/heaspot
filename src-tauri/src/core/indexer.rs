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

/// Xoá cache icon (dùng cho Clear cache trong Settings).
pub fn clear_icon_cache() {
    if let Some(c) = ICON_CACHE.get() {
        if let Ok(mut m) = c.lock() {
            m.clear();
        }
    }
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
            // Quét app trước và commit NGAY (nhanh) -> app hiện gần như tức thì,
            // không phải chờ scan_files (quét Desktop/Documents... có thể lâu).
            let apps = scan_apps();
            let state = app.state::<crate::AppState>();
            if let Ok(mut w) = state.apps.write() {
                *w = apps;
            }
            let files = scan_files();
            if let Ok(mut w) = state.files.write() {
                *w = files;
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

    // UWP / Store app (Notepad, Calculator, Settings...) — không có .lnk trong Start Menu
    scan_startapps(&mut apps, &mut seen);

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

/// Quét toàn bộ app hiển thị trong Start (gồm UWP/Store) qua `Get-StartApps`.
/// Mỗi app mở bằng `shell:AppsFolder\<AppID>`. Dedup theo tên với các app .lnk
/// đã quét trước (ưu tiên .lnk vì có sẵn icon thật). Với app UWP, resolve logo
/// thật từ AppxManifest (Square44x44Logo) rồi đọc PNG -> data URL cho frontend.
fn scan_startapps(apps: &mut Vec<AppEntry>, seen: &mut HashSet<String>) {
    // PowerShell: liệt kê Start apps + tìm file logo tốt nhất cho app UWP.
    // Đọc AppxManifest.xml trực tiếp (nhanh hơn Get-AppxPackageManifest), ưu tiên
    // asset có nền (tránh altform-unplated bị trong suốt/vô hình).
    let script = r#"
$loc = @{}
Get-AppxPackage | ForEach-Object { $loc[$_.PackageFamilyName] = $_.InstallLocation }
$out = foreach ($a in Get-StartApps) {
  $logo = $null
  if ($a.AppID -like '*!*') {
    $fam = $a.AppID.Split('!')[0]
    $l = $loc[$fam]
    if ($l) {
      $mf = Join-Path $l 'AppxManifest.xml'
      if (Test-Path -LiteralPath $mf) {
        try {
          [xml]$x = Get-Content -LiteralPath $mf -Raw
          $rel = $null
          $ve = $x.SelectSingleNode("//*[local-name()='VisualElements']")
          if ($ve) { $rel = $ve.GetAttribute('Square44x44Logo') }
          if (-not $rel) { $rel = $ve.GetAttribute('Square150x150Logo') }
          if (-not $rel) { $p = $x.SelectSingleNode("//*[local-name()='Logo']"); if ($p) { $rel = $p.'#text' } }
          if ($rel) {
            $base = Join-Path $l $rel
            $dir = Split-Path -Path $base
            $bn = [IO.Path]::GetFileNameWithoutExtension($base)
            $ex = [IO.Path]::GetExtension($base)
            if (Test-Path -LiteralPath $dir) {
              $cand = Get-ChildItem -LiteralPath $dir -Filter "$bn*$ex" -ErrorAction SilentlyContinue |
                Sort-Object @{e={ if($_.Name -match 'altform-unplated'){1}else{0} }},
                            @{e={ if($_.Name -match 'scale-200'){0}elseif($_.Name -match 'scale-100'){1}elseif($_.Name -match 'targetsize-4[48]'){2}else{3} }} |
                Select-Object -First 1
              if ($cand) { $logo = $cand.FullName }
            }
          }
        } catch {}
      }
    }
  }
  [pscustomobject]@{ Name=$a.Name; AppID=$a.AppID; Logo=$logo }
}
ConvertTo-Json -InputObject @($out) -Compress
"#;
    let Ok(out) = crate::commands::run_hidden_ps(script, &[]) else {
        return;
    };
    let out = out.trim();
    if out.is_empty() {
        return;
    }
    let Ok(val) = serde_json::from_str::<serde_json::Value>(out) else {
        return;
    };
    let items = match val {
        serde_json::Value::Array(a) => a,
        obj @ serde_json::Value::Object(_) => vec![obj],
        _ => return,
    };
    for it in items {
        let name = it
            .get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let appid = it
            .get("AppID")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() || appid.is_empty() {
            continue;
        }
        let lower = name.to_lowercase();
        if lower.contains("uninstall") || lower.contains("gỡ cài đặt") {
            continue;
        }
        let icon = it
            .get("Logo")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .and_then(png_file_to_data_url);
        if seen.insert(lower.clone()) {
            apps.push(AppEntry {
                name,
                path: format!("shell:AppsFolder\\{appid}"),
                icon,
            });
        } else if let Some(icon) = icon {
            // Start Menu có thể đã cung cấp .lnk cùng tên nhưng Shell không lấy
            // được icon (thường gặp với Notepad/Calculator UWP). Giữ path .lnk
            // để launch ổn định, nhưng thay bằng logo thật từ AppxManifest.
            if let Some(existing) = apps.iter_mut().find(|app| app.name.to_lowercase() == lower) {
                existing.icon = Some(icon);
            }
        }
    }
}

/// Đọc file PNG (logo UWP) -> data URL base64 để frontend hiển thị trực tiếp.
fn png_file_to_data_url(path: &str) -> Option<String> {
    use base64::Engine;
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 8 || bytes.len() > 512 * 1024 {
        return None;
    }
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    ))
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
