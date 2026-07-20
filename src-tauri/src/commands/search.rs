use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Thư mục resources chứa Everything bundled (set từ setup, trước lần search đầu)
static BUNDLED_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
static EVERYTHING_CHILD: OnceLock<Mutex<Option<std::process::Child>>> = OnceLock::new();

fn everything_child() -> &'static Mutex<Option<std::process::Child>> {
    EVERYTHING_CHILD.get_or_init(|| Mutex::new(None))
}

/// The bundled IPC engine is owned by HeaSpot and must never outlive it.
pub fn shutdown_everything() {
    if let Ok(mut slot) = everything_child().lock() {
        if let Some(mut child) = slot.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Gọi một lần lúc khởi động: đăng ký đường dẫn Everything bundled và
/// tự khởi chạy Everything.exe chạy nền nếu chưa có engine IPC nào đang chạy.
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
        if everything().is_none() {
            return; // không load được DLL -> dùng index nội bộ
        }
        let Some(evdir) = dir.filter(|d| d.join("Everything.exe").exists()) else {
            return;
        };
        let exe = evdir.join("Everything.exe");
        // Ghi Everything.ini vào data dir của app. Portable Everything không có
        // service/admin sẽ không đọc MFT, vì vậy dùng Folder Index cho user home:
        // vẫn có SDK/monitor realtime mà không cần UAC hay cài thêm service.
        let cfg_dir = crate::db::db_path()
            .parent()
            .map(|p| p.join("everything"))
            .unwrap_or_else(|| evdir.clone());
        let _ = std::fs::create_dir_all(&cfg_dir);
        let ini = cfg_dir.join("Everything.ini");
        let home = dirs::home_dir()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| r"C:\Users".into());
        let config = format!(
            "[Everything]\r\n\
             app_data=0\r\n\
             run_as_admin=0\r\n\
             run_in_background=1\r\n\
             show_tray_icon=0\r\n\
             show_in_taskbar=0\r\n\
             minimize_to_tray=0\r\n\
             ipc=1\r\n\
             start_in_background=1\r\n\
             update_notification=0\r\n\
             folders={home}\r\n\
             folder_monitor_changes=1\r\n\
             folder_buffer_size_list=65536\r\n\
             folder_rescan_if_full_list=1\r\n\
             folder_update_types=1\r\n\
             folder_update_intervals=10\r\n\
             folder_update_interval_types=0\r\n\
             folder_update_rescan_asap=1\r\n"
        );
        let needs_migration = std::fs::read_to_string(&ini)
            .map(|old| {
                !old.contains("[Everything]")
                    || !old.contains("folders=")
                    || !old.contains("show_tray_icon=0")
                    || !old.contains("run_as_admin=0")
            })
            .unwrap_or(true);
        let _ = std::fs::write(&ini, config);

        // v0.1.7 từng tạo INI thiếu section nên process app-owned có thể chạy với
        // index rỗng. Chỉ khi migrate, dừng đúng executable bundled của HeaSpot;
        // tuyệt đối không đụng instance Everything do người dùng cài riêng.
        if needs_migration {
            let script = r#"
$target = [IO.Path]::GetFullPath($env:HEASPOT_EVERYTHING_EXE)
Get-CimInstance Win32_Process -Filter "Name='Everything.exe'" -ErrorAction SilentlyContinue |
  Where-Object { $_.ExecutablePath -and ([IO.Path]::GetFullPath($_.ExecutablePath) -eq $target) } |
  ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
"#;
            let exe_text = exe.to_string_lossy().to_string();
            let _ = crate::commands::run_hidden_ps(
                script,
                &[("HEASPOT_EVERYTHING_EXE", &exe_text)],
            );
            std::thread::sleep(std::time::Duration::from_millis(250));
        }

        // Everything SDK 1.4 chỉ kết nối được IPC instance mặc định. Không gọi
        // Query để "probe" trước khi engine tồn tại vì QueryW(TRUE) có thể chờ
        // rất lâu. Cứ khởi động bản bundled; Everything tự thoát nếu một engine
        // mặc định khác đã tồn tại. -startup không hiện cửa sổ/tray.
        if let Ok(child) = std::process::Command::new(exe)
            .arg("-config")
            .arg(&ini)
            .arg("-startup")
            .spawn()
        {
            if let Ok(mut slot) = everything_child().lock() {
                *slot = Some(child);
            }
        }
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

#[derive(Serialize)]
pub struct ResultIcon {
    pub path: String,
    pub icon: Option<String>,
}

/// Resolve shell icons after textual results have already been rendered. This
/// preserves real file/folder icons without putting shell I/O on the typing
/// critical path.
#[tauri::command]
pub async fn load_result_icons(paths: Vec<String>) -> Result<Vec<ResultIcon>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        paths.into_iter().take(20).map(|path| ResultIcon {
            icon: crate::core::indexer::icon_for(&path),
            path,
        }).collect()
    }).await.map_err(|e| e.to_string())
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

/// Rank executable search hits as apps, while keeping the actual portable app
/// above similarly named installer packages such as `Foo_1.2_setup.exe`.
fn score_path_hit(path: &str, name: &str, query: &str, is_dir: bool, base: i32) -> (bool, i32) {
    let file = std::path::Path::new(path);
    let executable = !is_dir
        && file.extension().and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
    if !executable {
        return (false, base);
    }

    let stem = file.file_stem().and_then(|value| value.to_str()).unwrap_or(name);
    let stem_normalized = normalize_search(stem);
    let query_normalized = normalize_search(query.trim());
    let mut score = base.max(score_match(stem, query).unwrap_or(base)) + 10_000;
    if stem_normalized == query_normalized {
        score += 3_000;
    }
    let installer = stem_normalized.contains("setup") || stem_normalized.contains("installer");
    let asks_for_installer = query_normalized.contains("setup") || query_normalized.contains("install");
    if installer && !asks_for_installer {
        score -= 1_500;
    }
    (true, score)
}

/// Match names as users remember them, rather than only the shortcut label.
/// Stable aliases cover the common executable/product-name mismatch and
/// initials make queries such as "vsc" useful without weakening file search.
fn normalize_search(value: &str) -> String {
    value.to_lowercase().chars().map(|c| match c {
        'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ấ' | 'ầ' | 'ẩ' | 'ẫ' | 'ậ' => 'a',
        'đ' => 'd',
        'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => 'e',
        'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' => 'i',
        'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ớ' | 'ờ' | 'ở' | 'ỡ' | 'ợ' => 'o',
        'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => 'u',
        'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
        other => other,
    }).collect()
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ac) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, bc) in b.iter().enumerate() {
            cur[j + 1] = (cur[j] + 1).min(prev[j + 1] + 1).min(prev[j] + usize::from(ac != bc));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn score_app_match(app: &crate::core::indexer::AppEntry, query: &str) -> Option<i32> {
    let q = normalize_search(query.trim());
    let normalized = normalize_search(&app.name);
    let mut candidates = vec![normalized.clone()];
    let path = std::path::Path::new(&app.path);
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        candidates.push(normalize_search(stem));
    }
    if let Some(parent) = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()) {
        candidates.push(normalize_search(parent));
    }
    if let Some(app_id) = app.path.strip_prefix("shell:AppsFolder\\") {
        candidates.extend(app_id.split(['.', '_', '!', '-']).map(normalize_search));
    }
    let initials: String = normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter_map(|word| word.chars().next())
        .collect();
    candidates.push(initials.clone());
    candidates.push(normalized.chars().filter(|c| c.is_alphanumeric()).collect());
    candidates.sort();
    candidates.dedup();
    let mut best = candidates.iter().filter_map(|candidate| score_match(candidate, &q)).max();
    if initials == q {
        best = Some(best.unwrap_or(0).max(920));
    }
    // Generic typo tolerance: compare the query to every word in all indexed
    // names. One typo is accepted for 4–7 chars, two for longer queries.
    let tolerance = if q.len() >= 8 { 2 } else if q.len() >= 4 { 1 } else { 0 };
    if tolerance > 0 {
        for word in candidates.iter().flat_map(|c| c.split(|x: char| !x.is_alphanumeric())) {
            if !word.is_empty() && edit_distance(word, &q) <= tolerance {
                best = Some(best.unwrap_or(0).max(650 - edit_distance(word, &q) as i32 * 80));
            }
        }
    }
    // Multi-word fuzzy matching tolerates a typo in one token (for example
    // "visal studio") while still requiring every query token to match some
    // indexed token, avoiding broad/noisy semantic guesses.
    let query_words: Vec<&str> = q.split_whitespace().collect();
    if query_words.len() > 1 {
        let candidate_words: Vec<&str> = candidates
            .iter()
            .flat_map(|c| c.split(|x: char| !x.is_alphanumeric()))
            .filter(|w| !w.is_empty())
            .collect();
        let all_match = query_words.iter().all(|needle| {
            let allowed = if needle.len() >= 8 { 2 } else if needle.len() >= 4 { 1 } else { 0 };
            candidate_words.iter().any(|word| {
                word.starts_with(needle) || (allowed > 0 && edit_distance(word, needle) <= allowed)
            })
        });
        if all_match {
            best = Some(best.unwrap_or(0).max(760));
        }
    }
    best
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut it = haystack.chars();
    needle.chars().all(|c| it.any(|h| h == c))
}

/// Everything index cả kho thành phần Windows/Store. Các đường dẫn này không
/// phải file người dùng muốn mở từ launcher và tạo hàng chục kết quả trùng app.
fn is_search_noise(path: &str) -> bool {
    let p = path.replace('/', "\\").to_lowercase();
    [
        "\\windows\\winsxs\\",
        "\\windows\\servicing\\",
        "\\windows\\systemapps\\",
        "\\windows\\assembly\\",
        "\\windows\\installer\\",
        "\\program files\\windowsapps\\",
        "\\appdata\\local\\microsoft\\windowsapps\\",
        "\\programdata\\microsoft\\windows\\apprepository\\packages\\",
        "\\programdata\\packages\\",
        "\\$recycle.bin\\",
        "\\system volume information\\",
        "\\resources\\heaspot-everything\\",
    ]
    .iter()
    .any(|part| p.contains(part))
}

/// `in` là tìm nội dung tài liệu, không phải quét chuỗi nhị phân trong EXE/DLL.
fn is_fulltext_document(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "txt" | "md" | "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx"
            | "csv" | "rtf" | "log" | "ini" | "json" | "xml" | "html" | "htm"
            | "yaml" | "yml" | "toml" | "py" | "js" | "jsx" | "ts" | "tsx"
            | "rs" | "c" | "cc" | "cpp" | "h" | "hpp" | "java" | "cs" | "sql"
    )
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

    // Adaptive ranking: frequently/recently launched apps receive a modest
    // boost, never enough to make an unrelated app match.
    let usage: std::collections::HashMap<String, i32> = state.db.lock().ok().and_then(|conn| {
        let mut stmt = conn.prepare("SELECT path, launch_count FROM launch_usage").ok()?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))).ok()?;
        Some(rows.flatten().map(|(path, count)| (path.to_lowercase(), count)).collect())
    }).unwrap_or_default();

    // 1. Apps
    if let Ok(apps) = state.apps.read() {
        let mut scored: Vec<SearchResult> = apps
            .iter()
            .filter_map(|a| {
                score_app_match(a, &query).map(|s| SearchResult {
                    title: a.name.clone(),
                    subtitle: if a.path.starts_with("shell:AppsFolder\\") {
                        "Ứng dụng Windows".into()
                    } else {
                        a.path.clone()
                    },
                    kind: "app".into(),
                    path: a.path.clone(),
                    // App LUÔN xếp trên file/folder: cộng offset lớn hơn điểm khớp tối đa
                    // của file (~1000). Nhờ vậy "vscode" -> app "Visual Studio Code" (khớp
                    // mờ) vẫn thắng các folder ".vscode" khớp tên chính xác.
                    score: s + 10_000 + usage.get(&a.path.to_lowercase()).copied().unwrap_or(0).min(25) * 8,
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
    // Everything mặc định sắp xếp theo tên/path. Lấy một tập ứng viên đủ rộng
    // rồi tự chấm điểm; nếu chỉ xin 12 kết quả thì các folder/build metadata có
    // thể đẩy chính file `winspot.exe` (hoặc app portable khác) ra khỏi tập.
    if let Some(hits) = everything().as_ref().and_then(|e| e.search(&query, 200)) {
        engine = "everything".into();
        for (path, is_dir) in hits {
            if is_search_noise(&path) {
                continue;
            }
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            let score = score_match(&name, &query).unwrap_or(100);
            let (executable, score) = score_path_hit(&path, &name, &query, is_dir, score);
            results.push(SearchResult {
                title: name,
                subtitle: path.clone(),
                kind: if is_dir { "folder" } else if executable { "app" } else { "file" }.into(),
                path,
                score,
                icon: None,
            });
        }
    } else if let Ok(files) = state.files.read() {
        let mut scored: Vec<SearchResult> = files
            .iter()
            .filter_map(|f| {
                score_match(&f.name, &query).map(|s| {
                    let (executable, score) = score_path_hit(&f.path, &f.name, &query, f.is_dir, s);
                    SearchResult {
                    title: f.name.clone(),
                    subtitle: f.path.clone(),
                    kind: if f.is_dir { "folder" } else if executable { "app" } else { "file" }.into(),
                    path: f.path.clone(),
                    score,
                    icon: None,
                }})
            })
            .collect();
        scored.sort_by(|a, b| b.score.cmp(&a.score));
        scored.truncate(12);
        results.extend(scored);
    }

    results.sort_by(|a, b| b.score.cmp(&a.score));
    // Dedup theo path: Everything đôi khi trả cùng path 2 lần và app/file có thể
    // trùng path. Trùng path -> id `kind:path` ở frontend trùng -> React lẫn key
    // -> nhãn Ctrl+N gắn nhầm dòng và highlight lệch. Giữ bản điểm cao nhất.
    let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    results.retain(|r| seen_paths.insert(r.path.to_lowercase()));
    results.truncate(15);
    // File/folder icon extraction uses the Windows shell and can stall a
    // keystroke for hundreds of milliseconds on network/offline paths. Apps
    // already have cached icons; files intentionally use the instant generic
    // icon in the result list.
    SearchResponse { results, engine }
}

// ---------------------------------------------------------------------------
// Everything SDK qua FFI (tuỳ chọn — fallback sang index nội bộ nếu không có)
// ---------------------------------------------------------------------------

struct Everything {
    lib: libloading::Library,
}

/// Everything SDK giữ trạng thái truy vấn ở cấp process. Khóa để search tên file
/// và fallback `content:` của `in` không ghi đè truy vấn của nhau.
fn everything_query_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
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
        type IsDbLoaded = unsafe extern "system" fn() -> i32;
        type GetNumResults = unsafe extern "system" fn() -> u32;
        type GetFullPathW = unsafe extern "system" fn(u32, *mut u16, u32) -> u32;
        type IsFolderResult = unsafe extern "system" fn(u32) -> i32;

        let _query_guard = everything_query_lock().lock().ok()?;
        unsafe {
            let set_search: libloading::Symbol<SetSearchW> =
                self.lib.get(b"Everything_SetSearchW\0").ok()?;
            let set_max: libloading::Symbol<SetMax> =
                self.lib.get(b"Everything_SetMax\0").ok()?;
            let query_fn: libloading::Symbol<QueryW> =
                self.lib.get(b"Everything_QueryW\0").ok()?;
            let is_db_loaded: libloading::Symbol<IsDbLoaded> =
                self.lib.get(b"Everything_IsDBLoaded\0").ok()?;
            let num_results: libloading::Symbol<GetNumResults> =
                self.lib.get(b"Everything_GetNumResults\0").ok()?;
            let get_path: libloading::Symbol<GetFullPathW> =
                self.lib.get(b"Everything_GetResultFullPathNameW\0").ok()?;
            let is_folder: libloading::Symbol<IsFolderResult> =
                self.lib.get(b"Everything_IsFolderResult\0").ok()?;

            let wide: Vec<u16> = query.encode_utf16().chain(std::iter::once(0)).collect();
            // Khi app vừa khởi động, fallback sang index nội bộ thay vì block
            // nhịp gõ đầu tiên trong lúc Everything còn đang nạp database.
            if is_db_loaded() == 0 {
                return None;
            }
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
// Full-text hybrid: Windows Search trước, Everything content fallback
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct FullTextHit {
    pub name: String,
    pub path: String,
    pub preview: String,
}

#[derive(Serialize, Clone)]
pub struct FullTextResponse {
    pub results: Vec<FullTextHit>,
    /// `windows-search`, `everything-content` hoặc `hybrid` khi cả hai đều rỗng.
    pub engine: String,
}

/// Cache kết quả full-text theo phiên -> tra lại cùng từ khoá là tức thì.
fn fulltext_cache() -> &'static Mutex<std::collections::HashMap<String, FullTextResponse>> {
    static C: OnceLock<Mutex<std::collections::HashMap<String, FullTextResponse>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// Xoá cache full-text (dùng cho Clear cache trong Settings).
pub fn clear_fulltext_cache() {
    if let Ok(mut m) = fulltext_cache().lock() {
        m.clear();
    }
}

const FULLTEXT_EXTENSIONS: &str = "txt;md;pdf;doc;docx;xls;xlsx;ppt;pptx;csv;rtf;log;ini;json;xml;html;htm;yaml;yml;toml;py;js;jsx;ts;tsx;rs;c;cc;cpp;h;hpp;java;cs;sql";

fn everything_literal(value: &str) -> String {
    value.replace('"', "&quot:")
}

fn build_everything_content_query(query: &str, home: &str) -> String {
    // `content:` ở cuối để Everything lọc trước. Bỏ cache/build dependencies vì
    // quét iFilter từng file ở các cây này vừa chậm vừa cho kết quả ít giá trị.
    format!(
        "file: path:\"{}\" ext:{} !path:\"\\AppData\\\" !path:\"\\node_modules\\\" \
         !path:\"\\.git\\\" !path:\"\\target\\\" !path:\"\\dist\\\" content:\"{}\"",
        everything_literal(home),
        FULLTEXT_EXTENSIONS,
        everything_literal(query)
    )
}

fn content_preview(path: &str, query: &str) -> String {
    let Ok(meta) = std::fs::metadata(path) else {
        return String::new();
    };
    if meta.len() > 4 * 1024 * 1024 {
        return String::new();
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let needle = query.to_lowercase();
    text.lines()
        .find(|line| line.to_lowercase().contains(&needle))
        .map(|line| line.trim().chars().take(240).collect())
        .unwrap_or_default()
}

/// Tìm nội dung bằng chiến lược hybrid: Windows Search đã index cho phản hồi nhanh,
/// rồi fallback Everything `content:` để phủ file ngoài vùng index.
#[tauri::command]
pub async fn fulltext_search(query: String) -> Result<FullTextResponse, String> {
    let q: String = query
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let q = q.trim().to_string();
    if q.is_empty() {
        return Ok(FullTextResponse {
            results: Vec::new(),
            engine: "hybrid".into(),
        });
    }
    let key = q.to_lowercase();
    if let Some(hits) = fulltext_cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Ok(hits);
    }

    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| "C:/Users".into());

    // CONTAINS(System.Search.Contents, ...) -> chỉ tìm trong NỘI DUNG file
    // (không khớp tên file/app như CONTAINS(*, ...) trước đây gây "app lạ nhảy vào").
    let sql = format!(
        "SELECT TOP 15 System.ItemName, System.ItemPathDisplay, System.Search.AutoSummary \
         FROM SystemIndex \
         WHERE SCOPE='file:{home}' AND CONTAINS(System.Search.Contents, '\"{q}*\"') \
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
    .map_err(|e| e.to_string())?
    .unwrap_or_default();

    let json = stdout.trim();
    let value: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::json!([]));
    let items = match value {
        serde_json::Value::Array(a) => a,
        obj @ serde_json::Value::Object(_) => vec![obj],
        _ => vec![],
    };
    let mut hits = items
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
        .filter(|h| !h.path.is_empty() && is_fulltext_document(&h.path) && !is_search_noise(&h.path))
        .collect::<Vec<_>>();

    let engine = if hits.is_empty() {
        let home = dirs::home_dir()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|| r"C:\Users".into());
        let everything_query = build_everything_content_query(&q, &home);
        let query_for_preview = q.clone();
        let paths = tauri::async_runtime::spawn_blocking(move || {
            everything()
                .as_ref()
                .and_then(|engine| engine.search(&everything_query, 15))
        })
        .await
        .map_err(|e| e.to_string())?;
        if let Some(paths) = paths {
            hits = paths
                .into_iter()
                .filter(|(path, is_dir)| {
                    !is_dir && is_fulltext_document(path) && !is_search_noise(path)
                })
                .map(|(path, _)| {
                    let name = std::path::Path::new(&path)
                        .file_name()
                        .map(|value| value.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.clone());
                    FullTextHit {
                        name,
                        preview: content_preview(&path, &query_for_preview),
                        path,
                    }
                })
                .collect();
        }
        if hits.is_empty() {
            "hybrid"
        } else {
            "everything-content"
        }
    } else {
        "windows-search"
    };

    let response = FullTextResponse {
        results: hits,
        engine: engine.into(),
    };
    // Chỉ cache khi có kết quả (tránh kẹt cache rỗng do lỗi nhất thời).
    if !response.results.is_empty() {
        if let Ok(mut c) = fulltext_cache().lock() {
            if c.len() >= 300 {
                c.clear();
            }
            c.insert(key, response.clone());
        }
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::{build_everything_content_query, is_fulltext_document, is_search_noise, score_app_match, score_path_hit};

    #[test]
    fn app_aliases_match_product_names() {
        let code = crate::core::indexer::AppEntry {
            name: "Visual Studio Code".into(),
            path: r"C:\Apps\Microsoft VS Code\Code.exe".into(),
            icon: None,
        };
        let edge = crate::core::indexer::AppEntry {
            name: "Microsoft Edge".into(),
            path: r"C:\Apps\Edge\msedge.exe".into(),
            icon: None,
        };
        assert!(score_app_match(&code, "code").unwrap() >= 950);
        assert!(score_app_match(&code, "vscode").is_some());
        assert!(score_app_match(&code, "vsc").is_some());
        assert!(score_app_match(&code, "visal studio").is_some());
        assert!(score_app_match(&edge, "edge").is_some());
        assert!(score_app_match(&code, "unrelated folder").is_none());
    }

    #[test]
    fn exact_portable_app_ranks_above_its_installer_and_folder() {
        let (_, portable) = score_path_hit(
            r"C:\Apps\winspot.exe",
            "winspot.exe",
            "winspot",
            false,
            800,
        );
        let (_, installer) = score_path_hit(
            r"C:\Downloads\WinSpot_0.1.2_x64-setup.exe",
            "WinSpot_0.1.2_x64-setup.exe",
            "winspot",
            false,
            800,
        );
        let (_, folder) = score_path_hit(
            r"C:\Projects\winspot",
            "winspot",
            "winspot",
            true,
            1_000,
        );
        assert!(portable > installer);
        assert!(portable > folder);
    }

    #[test]
    fn filters_windows_component_and_store_package_paths() {
        assert!(is_search_noise(r"C:\Windows\WinSxS\amd64_notepad.resources"));
        assert!(is_search_noise(r"C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_1.0"));
        assert!(!is_search_noise(r"C:\Users\Lan\Documents\notepad-notes.txt"));
    }

    #[test]
    fn fulltext_accepts_documents_but_rejects_binaries() {
        assert!(is_fulltext_document(r"C:\Users\Lan\Documents\report.docx"));
        assert!(is_fulltext_document(r"C:\Users\Lan\code\main.rs"));
        assert!(!is_fulltext_document(r"C:\Program Files\Internet Explorer\IEDIAGCMD.EXE"));
        assert!(!is_fulltext_document(r"C:\Windows\System32\helper.dll"));
    }

    #[test]
    fn everything_fallback_filters_before_content_scan() {
        let query = build_everything_content_query("học tập", r"C:\Users\Lan");
        assert!(query.starts_with(r#"file: path:"C:\Users\Lan" ext:"#));
        assert!(query.ends_with(r#"content:"học tập""#));
    }
}
