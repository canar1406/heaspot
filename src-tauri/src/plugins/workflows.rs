//! Alfred Workflow Compatibility (MVP).
//!
//! `.alfredworkflow` = file ZIP chứa `info.plist` (XML Property List) + script.
//! WinSpot hỗ trợ NHÓM TƯƠNG THÍCH CAO: Script Filter viết bằng
//! Python / Node.js / PHP / Ruby / Bash (Git Bash). AppleScript -> báo không hỗ trợ.
//!
//! Cài đặt: gõ `workflow install <đường dẫn .alfredworkflow>` trên thanh search,
//! hoặc chép file vào %APPDATA%\winspot\workflows rồi gõ `workflow rescan`.

use base64::Engine;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

#[derive(Serialize, Clone)]
pub struct WorkflowInfo {
    pub name: String,
    pub keyword: String,
    pub dir: String,
    pub script: String,      // inline script hoặc rỗng
    pub script_file: String, // file script hoặc rỗng
    pub script_type: String, // "python" | "node" | "php" | "ruby" | "bash" | "unsupported"
    pub icon: Option<String>,
}

#[derive(Serialize)]
pub struct WorkflowItem {
    pub title: String,
    pub subtitle: String,
    pub arg: String,
}

fn store() -> &'static RwLock<Vec<WorkflowInfo>> {
    static S: OnceLock<RwLock<Vec<WorkflowInfo>>> = OnceLock::new();
    S.get_or_init(|| RwLock::new(scan_workflows()))
}

pub fn workflows_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("heaspot").join("workflows")
}

/// Map "type" trong info.plist / đuôi file script -> runner trên Windows
fn runner_for(type_num: Option<i64>, script_file: &str) -> String {
    if !script_file.is_empty() {
        let ext = Path::new(script_file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        return match ext.as_str() {
            "py" => "python",
            "js" | "mjs" => "node",
            "php" => "php",
            "rb" => "ruby",
            "sh" => "bash",
            "pl" => "perl",
            _ => "unsupported",
        }
        .to_string();
    }
    match type_num {
        Some(0) | Some(5) => "bash", // bash / zsh -> Git Bash nếu có
        Some(1) => "php",
        Some(2) => "ruby",
        Some(3) => "python",
        Some(4) => "perl",
        _ => "unsupported", // 6/7 = AppleScript/JXA
    }
    .to_string()
}

fn parse_workflow(dir: &Path) -> Option<WorkflowInfo> {
    let plist_path = dir.join("info.plist");
    let v: plist::Value = plist::from_file(&plist_path).ok()?;
    let dict = v.as_dictionary()?;
    let name = dict
        .get("name")
        .and_then(|n| n.as_string())
        .unwrap_or("workflow")
        .to_string();

    let objects = dict.get("objects")?.as_array()?;
    // Tìm Script Filter (hoặc keyword input) đầu tiên có keyword
    for obj in objects {
        let od = match obj.as_dictionary() {
            Some(d) => d,
            None => continue,
        };
        let otype = od.get("type").and_then(|t| t.as_string()).unwrap_or("");
        if !otype.contains("input.scriptfilter") {
            continue;
        }
        let Some(config) = od.get("config").and_then(|c| c.as_dictionary()) else {
            continue;
        };
        let keyword = config
            .get("keyword")
            .and_then(|k| k.as_string())
            .unwrap_or("")
            .trim()
            .to_string();
        if keyword.is_empty() {
            continue;
        }
        let script = config
            .get("script")
            .and_then(|s| s.as_string())
            .unwrap_or("")
            .to_string();
        let script_file = config
            .get("scriptfile")
            .and_then(|s| s.as_string())
            .unwrap_or("")
            .to_string();
        let type_num = config.get("type").and_then(|t| t.as_signed_integer());

        let icon = std::fs::read(dir.join("icon.png")).ok().map(|bytes| {
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            )
        });

        return Some(WorkflowInfo {
            name,
            keyword,
            dir: dir.to_string_lossy().to_string(),
            script_type: runner_for(type_num, &script_file),
            script,
            script_file,
            icon,
        });
    }
    None
}

pub fn scan_workflows() -> Vec<WorkflowInfo> {
    let dir = workflows_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Some(wf) = parse_workflow(&p) {
                    out.push(wf);
                }
            }
        }
    }
    out
}

#[tauri::command]
pub fn list_workflows() -> Vec<WorkflowInfo> {
    store().read().map(|v| v.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn rescan_workflows() -> Vec<WorkflowInfo> {
    let fresh = scan_workflows();
    if let Ok(mut w) = store().write() {
        *w = fresh.clone();
    }
    fresh
}

/// Giải nén file .alfredworkflow (bản chất là ZIP) vào thư mục workflows
#[tauri::command]
pub fn install_workflow(path: String) -> Result<String, String> {
    let src = PathBuf::from(path.trim().trim_matches('"'));
    if !src.exists() {
        return Err("file không tồn tại".into());
    }
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "workflow".into());
    let dest = workflows_dir().join(&stem);
    let _ = std::fs::create_dir_all(&dest);

    let file = std::fs::File::open(&src).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("không phải file ZIP hợp lệ: {e}"))?;
    zip.extract(&dest).map_err(|e| e.to_string())?;

    let wf = parse_workflow(&dest)
        .ok_or("không tìm thấy Script Filter có keyword trong info.plist (workflow này chưa được hỗ trợ)")?;
    if wf.script_type == "unsupported" {
        return Err(format!(
            "workflow \"{}\" dùng AppleScript/định dạng chỉ có trên macOS — không hỗ trợ",
            wf.name
        ));
    }
    let msg = format!("Đã cài \"{}\" — gõ `{} <từ khoá>` để dùng", wf.name, wf.keyword);
    rescan_workflows();
    Ok(msg)
}

/// Chạy Script Filter của workflow với query, parse JSON kết quả chuẩn Alfred
#[tauri::command]
pub async fn run_workflow(keyword: String, query: String) -> Result<Vec<WorkflowItem>, String> {
    let wf = {
        let list = store().read().map_err(|e| e.to_string())?;
        list.iter().find(|w| w.keyword == keyword).cloned()
    }
    .ok_or("không tìm thấy workflow")?;

    if wf.script_type == "unsupported" {
        return Err("workflow này dùng AppleScript — chỉ chạy được trên macOS".into());
    }

    let output = tauri::async_runtime::spawn_blocking(move || run_script(&wf, &query))
        .await
        .map_err(|e| e.to_string())??;

    // Alfred Script Filter JSON: { "items": [ { title, subtitle, arg } ] }
    let v: serde_json::Value = serde_json::from_str(output.trim())
        .map_err(|_| "script không trả về JSON chuẩn Alfred (có thể là workflow XML cũ)".to_string())?;
    let items = v
        .get("items")
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(items
        .into_iter()
        .filter_map(|it| {
            let title = it.get("title").and_then(|t| t.as_str())?.to_string();
            let subtitle = it
                .get("subtitle")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let arg = match it.get("arg") {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(serde_json::Value::Array(a)) => a
                    .iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
                _ => String::new(),
            };
            Some(WorkflowItem { title, subtitle, arg })
        })
        .take(15)
        .collect())
}

fn run_script(wf: &WorkflowInfo, query: &str) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let runner = wf.script_type.as_str();
    let dir = PathBuf::from(&wf.dir);

    // Script inline: thay {query} rồi ghi ra file tạm trong thư mục workflow
    let script_path = if !wf.script_file.is_empty() {
        dir.join(&wf.script_file)
    } else {
        let ext = match runner {
            "python" => "py",
            "node" => "js",
            "php" => "php",
            "ruby" => "rb",
            "perl" => "pl",
            _ => "sh",
        };
        let body = wf.script.replace("{query}", &query.replace('"', ""));
        let tmp = dir.join(format!(".winspot_run.{ext}"));
        std::fs::write(&tmp, body).map_err(|e| e.to_string())?;
        tmp
    };

    let program = match runner {
        "python" => "python",
        "node" => "node",
        "php" => "php",
        "ruby" => "ruby",
        "perl" => "perl",
        "bash" => "bash",
        _ => return Err("runner không hỗ trợ".into()),
    };

    let output = std::process::Command::new(program)
        .arg(&script_path)
        .arg(query)
        .current_dir(&dir)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| {
            format!("không chạy được `{program}` — hãy cài môi trường này và thêm vào PATH ({e})")
        })?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
