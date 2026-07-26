//! Tìm nội dung trong note Capacities qua API chính thức (api.capacities.io).
//! Token do người dùng cấp (Capacities → Settings → Capacities API),
//! lưu trong bảng settings của SQLite.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Serialize)]
pub struct CapacitiesHit {
    pub id: String,
    pub space_id: String,
    pub title: String,
    pub preview: String,
}

/// Cache danh sách spaceIds theo token (ít khi đổi) -> bỏ 1 request GET mỗi lần tìm.
fn spaces_cache() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static C: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Xoá cache spaceIds Capacities (dùng cho Clear cache trong Settings).
pub fn clear_spaces_cache() {
    if let Ok(mut m) = spaces_cache().lock() {
        m.clear();
    }
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
    .map_err(|e| e.to_string())?;
    if let Ok(mut c) = spaces_cache().lock() {
        c.clear();
    }
    Ok(())
}

#[tauri::command]
pub fn capacities_has_token(state: tauri::State<'_, crate::AppState>) -> bool {
    state.db.lock().ok().and_then(|c| read_token(&c)).is_some()
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

    // Native ureq: lấy spaceIds (cache) rồi POST search — không spawn PowerShell.
    let hits = tauri::async_runtime::spawn_blocking(move || capacities_call(&token, &q))
        .await
        .map_err(|e| e.to_string())?;
    Ok(hits.unwrap_or_default())
}

fn capacities_call(token: &str, q: &str) -> Option<Vec<CapacitiesHit>> {
    let bearer = format!("Bearer {token}");

    // spaceIds từ cache, nếu chưa có thì gọi GET /spaces một lần
    let ids: Vec<String> = if let Some(c) = spaces_cache()
        .lock()
        .ok()
        .and_then(|c| c.get(token).cloned())
    {
        c
    } else {
        let v: serde_json::Value = ureq::get("https://api.capacities.io/spaces")
            .set("Authorization", &bearer)
            .timeout(Duration::from_secs(8))
            .call()
            .ok()?
            .into_json()
            .ok()?;
        let ids: Vec<String> = v
            .get("spaces")
            .and_then(|s| s.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("id").and_then(|i| i.as_str()).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if !ids.is_empty() {
            if let Ok(mut c) = spaces_cache().lock() {
                c.insert(token.to_string(), ids.clone());
            }
        }
        ids
    };
    if ids.is_empty() {
        return Some(Vec::new());
    }

    let body = serde_json::json!({ "mode": "fullText", "searchTerm": q, "spaceIds": ids });
    let r: serde_json::Value = ureq::post("https://api.capacities.io/search")
        .set("Authorization", &bearer)
        .timeout(Duration::from_secs(10))
        .send_json(body)
        .ok()?
        .into_json()
        .ok()?;

    let results = r
        .get("results")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let hits = results
        .into_iter()
        .filter_map(|v| {
            let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let mut snips: Vec<String> = Vec::new();
            if let Some(hls) = v.get("highlights").and_then(|x| x.as_array()) {
                for hl in hls {
                    if let Some(arr) = hl.get("snippets").and_then(|x| x.as_array()) {
                        for s in arr {
                            if let Some(s) = s.as_str() {
                                snips.push(s.to_string());
                            }
                        }
                    }
                }
            }
            let preview = snips.into_iter().take(3).collect::<Vec<_>>().join(" … ");
            let hit = CapacitiesHit {
                id: get("id"),
                space_id: get("spaceId"),
                title: get("title"),
                preview,
            };
            (!hit.id.is_empty() && !hit.space_id.is_empty()).then_some(hit)
        })
        .take(10)
        .collect();
    Some(hits)
}
