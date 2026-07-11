use serde::Serialize;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Thư mục resources chứa Everything bundled (set từ setup, trước lần search đầu)
static BUNDLED_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Gọi một lần lúc khởi động: đăng ký đường dẫn Everything bundled và
/// tự khởi chạy Everything.exe chạy nền nếu chưa có instance nào đang chạy.
pub fn init_everything(app: &tauri::AppHandle) {
    use tauri::Manager;
    let dir = app
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("everything"))
        .filter(|d| d.exists());
    let _ = BUNDLED_DIR.set(dir.clone());

    std::thread::spawn(move || {
        let Some(ev) = everything().as_ref() else {
            return; // không load được DLL -> dùng index nội bộ
        };
        // Probe IPC: query được nghĩa là Everything (của user hoặc của app) đang chạy
        if ev.search("winspot::ipc::probe", 1).is_some() {
            return;
        }
        let Some(evdir) = dir.filter(|d| d.join("Everything.exe").exists()) else {
            return;
        };
        let exe = evdir.join("Everything.exe");
        // Ghi Everything.ini (tray_icon=0 -> KHÔNG hiện tray, chạy nền hoàn toàn)
        // vào %APPDATA%\heaspot\everything để chắc chắn ghi được (resources có thể read-only).
        let cfg_dir = crate::db::db_path()
            .parent()
            .map(|p| p.join("everything"))
            .unwrap_or_else(|| evdir.clone());
        let _ = std::fs::create_dir_all(&cfg_dir);
        let ini = cfg_dir.join("Everything.ini");
        if !ini.exists() {
            let _ = std::fs::write(
                &ini,
                "tray_icon=0\r\nrun_in_background=1\r\nstart_in_background=1\r\nupdate_notification=0\r\n",
            );
        }
        // -config trỏ tới ini của ta; -startup: chạy nền ngay, không hiện cửa sổ.
        let _ = std::process::Command::new(exe)
            .arg("-config")
            .arg(&ini)
            .arg("-startup")
            .spawn();
    });
}

#[derive(Serialize, Clone)]
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub kind: String, // "app" | "file" | "folder"
    pub path: String,
    pub score: i32,
    /// Icon thật (data URL PNG) — hiện chỉ có với app
    pub icon: Option<String>,
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub engine: String, // "everything" | "internal"
}

/// Chấm điểm khớp fuzzy đơn giản: prefix > word-boundary > substring > subsequence
fn score_match(name: &str, query: &str) -> Option<i32> {
    let name_l = name.to_lowercase();
    let q = query.to_lowercase();
    if q.is_empty() {
        return None;
    }
    let base = if name_l == q {
        1000
    } else if name_l.starts_with(&q) {
        800
    } else if name_l
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w.starts_with(&q))
    {
        700
    } else if name_l.contains(&q) {
        500
    } else if is_subsequence(&q, &name_l) {
        200
    } else {
        return None;
    };
    // Tên càng ngắn càng liên quan
    Some(base - (name_l.len().min(60) as i32) / 4)
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut it = haystack.chars();
    needle.chars().all(|c| it.any(|h| h == c))
}

/// Tìm kiếm chính: App (Start Menu/Registry) + File (Everything hoặc index nội bộ)
#[tauri::command]
pub fn search_all(query: String, state: tauri::State<'_, crate::AppState>) -> SearchResponse {
    let query = query.trim().to_string();
    let mut results: Vec<SearchResult> = Vec::new();
    if query.is_empty() {
        return SearchResponse {
            results,
            engine: "internal".into(),
        };
    }

    // 1. Apps
    if let Ok(apps) = state.apps.read() {
        let mut scored: Vec<SearchResult> = apps
            .iter()
            .filter_map(|a| {
                score_match(&a.name, &query).map(|s| SearchResult {
                    title: a.name.clone(),
                    subtitle: a.path.clone(),
                    kind: "app".into(),
                    path: a.path.clone(),
                    score: s + 100, // ưu tiên app hơn file
                    icon: a.icon.clone(),
                })
            })
            .collect();
        scored.sort_by(|a, b| b.score.cmp(&a.score));
        scored.truncate(6);
        results.extend(scored);
    }

    // 2. Files: ưu tiên Everything nếu có
    let mut engine = "internal".to_string();
    if let Some(hits) = everything().as_ref().and_then(|e| e.search(&query, 12)) {
        engine = "everything".into();
        for (path, is_dir) in hits {
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            let score = score_match(&name, &query).unwrap_or(100);
            results.push(SearchResult {
                title: name,
                subtitle: path.clone(),
                kind: if is_dir { "folder" } else { "file" }.into(),
                path,
                score,
                icon: None,
            });
        }
    } else if let Ok(files) = state.files.read() {
        let mut scored: Vec<SearchResult> = files
            .iter()
            .filter_map(|f| {
                score_match(&f.name, &query).map(|s| SearchResult {
                    title: f.name.clone(),
                    subtitle: f.path.clone(),
                    kind: if f.is_dir { "folder" } else { "file" }.into(),
                    path: f.path.clone(),
                    score: s,
                    icon: None,
                })
            })
            .collect();
        scored.sort_by(|a, b| b.score.cmp(&a.score));
        scored.truncate(12);
        results.extend(scored);
    }

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.truncate(15);
    // Trích icon thật cho file/folder (chỉ tập kết quả cuối, có cache theo path)
    for r in results.iter_mut() {
        if r.icon.is_none() && (r.kind == "file" || r.kind == "folder") {
            r.icon = crate::core::indexer::icon_for(&r.path);
        }
    }
    SearchResponse { results, engine }
}

// ---------------------------------------------------------------------------
// Everything SDK qua FFI (tuỳ chọn — fallback sang index nội bộ nếu không có)
// ---------------------------------------------------------------------------

struct Everything {
    lib: libloading::Library,
}

// Library chỉ được dùng qua các symbol lookup có khoá bởi OnceLock
unsafe impl Send for Everything {}
unsafe impl Sync for Everything {}

fn everything() -> &'static Option<Everything> {
    static INSTANCE: OnceLock<Option<Everything>> = OnceLock::new();
    INSTANCE.get_or_init(Everything::load)
}

impl Everything {
    fn load() -> Option<Self> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        // Ưu tiên DLL bundled sẵn trong resources của app
        if let Some(Some(dir)) = BUNDLED_DIR.get() {
            candidates.push(dir.join("Everything64.dll"));
        }
        candidates.push(PathBuf::from("Everything64.dll"));
        candidates.push(PathBuf::from(r"C:\Program Files\Everything\Everything64.dll"));
        candidates.push(PathBuf::from(
            r"C:\Program Files\Everything\SDK\dll\Everything64.dll",
        ));
        for c in candidates {
            if let Ok(lib) = unsafe { libloading::Library::new(&c) } {
                return Some(Everything { lib });
            }
        }
        None
    }

    fn search(&self, query: &str, max: u32) -> Option<Vec<(String, bool)>> {
        type SetSearchW = unsafe extern "system" fn(*const u16);
        type SetMax = unsafe extern "system" fn(u32);
        type QueryW = unsafe extern "system" fn(i32) -> i32;
        type GetNumResults = unsafe extern "system" fn() -> u32;
        type GetFullPathW = unsafe extern "system" fn(u32, *mut u16, u32) -> u32;
        type IsFolderResult = unsafe extern "system" fn(u32) -> i32;

        unsafe {
            let set_search: libloading::Symbol<SetSearchW> =
                self.lib.get(b"Everything_SetSearchW\0").ok()?;
            let set_max: libloading::Symbol<SetMax> =
                self.lib.get(b"Everything_SetMax\0").ok()?;
            let query_fn: libloading::Symbol<QueryW> =
                self.lib.get(b"Everything_QueryW\0").ok()?;
            let num_results: libloading::Symbol<GetNumResults> =
                self.lib.get(b"Everything_GetNumResults\0").ok()?;
            let get_path: libloading::Symbol<GetFullPathW> =
                self.lib.get(b"Everything_GetResultFullPathNameW\0").ok()?;
            let is_folder: libloading::Symbol<IsFolderResult> =
                self.lib.get(b"Everything_IsFolderResult\0").ok()?;

            let wide: Vec<u16> = query.encode_utf16().chain(std::iter::once(0)).collect();
            set_search(wide.as_ptr());
            set_max(max);
            // TRUE = chờ kết quả (Everything service phải đang chạy)
            if query_fn(1) == 0 {
                return None;
            }
            let n = num_results();
            let mut out = Vec::with_capacity(n as usize);
            let mut buf = vec![0u16; 4096];
            for i in 0..n {
                let len = get_path(i, buf.as_mut_ptr(), buf.len() as u32);
                if len == 0 {
                    continue;
                }
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                out.push((path, is_folder(i) != 0));
            }
            Some(out)
        }
    }
}

// ---------------------------------------------------------------------------
// Full-text search qua Windows Search (Search.CollatorDSO OLE DB)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct FullTextHit {
    pub name: String,
    pub path: String,
    pub preview: String,
}

/// Tìm nội dung bên trong file (Word, PDF, Excel...) qua Windows Search Index.
/// Chạy PowerShell ẩn để query OLE DB — tránh phải bind COM trực tiếp.
#[tauri::command]
pub async fn fulltext_search(query: String) -> Result<Vec<FullTextHit>, String> {
    let q: String = query
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let q = q.trim().to_string();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| "C:/Users".into());

    let sql = format!(
        "SELECT TOP 15 System.ItemName, System.ItemPathDisplay, System.Search.AutoSummary \
         FROM SystemIndex \
         WHERE SCOPE='file:{home}' AND CONTAINS(*, '\"{q}*\"') \
         ORDER BY System.Search.Rank DESC"
    );

    let script = r#"
try {
  $c = New-Object System.Data.OleDb.OleDbConnection "Provider=Search.CollatorDSO;Extended Properties='Application=Windows'"
  $c.Open()
  $cmd = $c.CreateCommand()
  $cmd.CommandText = $env:WINSPOT_SQL
  $r = $cmd.ExecuteReader()
  $out = New-Object System.Collections.ArrayList
  while ($r.Read()) {
    [void]$out.Add([pscustomobject]@{
      name = [string]$r.GetValue(0)
      path = [string]$r.GetValue(1)
      preview = [string]$r.GetValue(2)
    })
  }
  $r.Close(); $c.Close()
  ConvertTo-Json -InputObject @($out) -Compress
} catch { '[]' }
"#;

    let stdout = tauri::async_runtime::spawn_blocking(move || {
        crate::commands::run_hidden_ps(script, &[("WINSPOT_SQL", &sql)])
    })
    .await
    .map_err(|e| e.to_string())??;

    let json = stdout.trim();
    if json.is_empty() {
        return Ok(Vec::new());
    }
    let value: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::json!([]));
    let items = match value {
        serde_json::Value::Array(a) => a,
        obj @ serde_json::Value::Object(_) => vec![obj],
        _ => vec![],
    };
    let hits = items
        .into_iter()
        .filter_map(|v| {
            Some(FullTextHit {
                name: v.get("name")?.as_str().unwrap_or("").to_string(),
                path: v.get("path")?.as_str().unwrap_or("").to_string(),
                preview: v
                    .get("preview")
                    .and_then(|p| p.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            })
        })
        .filter(|h| !h.path.is_empty())
        .collect();
    Ok(hits)
}
