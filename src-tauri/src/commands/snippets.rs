use rusqlite::params;
use serde::Serialize;

#[derive(Serialize)]
pub struct Snippet {
    pub id: i64,
    pub keyword: String,
    pub content: String,
}

#[tauri::command]
pub fn get_snippets(state: tauri::State<'_, crate::AppState>) -> Result<Vec<Snippet>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, keyword, content FROM snippets ORDER BY keyword")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Snippet {
                id: r.get(0)?,
                keyword: r.get(1)?,
                content: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

/// Thêm/ghi đè snippet: gõ `snip <keyword> <nội dung>` trên thanh search
#[tauri::command]
pub fn add_snippet(
    keyword: String,
    content: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let keyword = keyword.trim().to_string();
    let content = content.trim().to_string();
    if keyword.is_empty() || content.is_empty() {
        return Err("keyword và nội dung không được rỗng".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO snippets (keyword, content) VALUES (?1, ?2) \
         ON CONFLICT(keyword) DO UPDATE SET content = excluded.content",
        params![keyword, content],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_snippet(id: i64, state: tauri::State<'_, crate::AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM snippets WHERE id = ?1", params![id])
        .map(|_| ())
        .map_err(|e| e.to_string())
}
