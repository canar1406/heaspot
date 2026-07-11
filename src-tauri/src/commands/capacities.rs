//! Tìm nội dung trong note Capacities qua API chính thức (api.capacities.io).
//! Token do người dùng cấp (Capacities → Settings → Capacities API),
//! lưu trong bảng settings của SQLite.

use serde::Serialize;

#[derive(Serialize)]
pub struct CapacitiesHit {
    pub id: String,
    pub space_id: String,
    pub title: String,
    pub preview: String,
}

fn read_token(conn: &rusqlite::Connection) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = 'capacities_token'",
        [],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .filter(|t| !t.trim().is_empty())
}

#[tauri::command]
pub fn set_capacities_token(
    token: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('capacities_token', ?1) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![token.trim()],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn capacities_has_token(state: tauri::State<'_, crate::AppState>) -> bool {
    state
        .db
        .lock()
        .ok()
        .and_then(|c| read_token(&c))
        .is_some()
}

#[tauri::command]
pub async fn capacities_search(
    query: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<CapacitiesHit>, String> {
    let token = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        read_token(&conn)
    };
    let Some(token) = token else {
        return Ok(Vec::new());
    };
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    // Gọi API qua PowerShell ẩn (đồng bộ với cách làm của fulltext_search)
    let script = r#"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
try {
  $H = @{ Authorization = "Bearer $env:CAP_TOKEN" }
  $spaces = (Invoke-RestMethod -Uri "https://api.capacities.io/spaces" -Headers $H -Method GET -TimeoutSec 8).spaces
  $ids = @($spaces | ForEach-Object { $_.id })
  $body = @{ mode = "fullText"; searchTerm = $env:CAP_QUERY; spaceIds = $ids } | ConvertTo-Json
  $r = Invoke-RestMethod -Uri "https://api.capacities.io/search" -Headers $H -Method POST -Body $body -ContentType "application/json" -TimeoutSec 10
  $out = @($r.results | ForEach-Object {
    $snips = @()
    foreach ($hl in @($_.highlights)) { $snips += @($hl.snippets) }
    [pscustomobject]@{
      id      = [string]$_.id
      spaceId = [string]$_.spaceId
      title   = [string]$_.title
      preview = [string](($snips | Select-Object -First 3) -join " … ")
    }
  })
  ConvertTo-Json -InputObject $out -Compress -Depth 5
} catch { '[]' }
"#;

    let output = tauri::async_runtime::spawn_blocking(move || {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("CAP_TOKEN", &token)
            .env("CAP_QUERY", &q)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
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
            let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let hit = CapacitiesHit {
                id: get("id"),
                space_id: get("spaceId"),
                title: get("title"),
                preview: get("preview"),
            };
            (!hit.id.is_empty() && !hit.space_id.is_empty()).then_some(hit)
        })
        .take(10)
        .collect();
    Ok(hits)
}
