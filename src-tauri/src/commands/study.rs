use rusqlite::params;
use serde::Serialize;

#[derive(Serialize)]
pub struct StudyWord {
    pub id: i64,
    pub word: String,
    pub translation: String,
    pub details: String,
    pub created_at: String,
}

#[tauri::command]
pub fn save_study_word(
    word: String,
    translation: String,
    details: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let word = word.trim();
    if word.is_empty() || translation.trim().is_empty() {
        return Err("từ và nghĩa không được trống".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO study_words(word,translation,details) VALUES(?1,?2,?3) \
         ON CONFLICT(word) DO UPDATE SET translation=excluded.translation, \
         details=excluded.details, created_at=datetime('now','localtime')",
        params![word, translation.trim(), details],
    ).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_study_words(
    filter: Option<String>,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<StudyWord>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let pattern = format!("%{}%", filter.unwrap_or_default());
    let mut stmt = conn.prepare(
        "SELECT id,word,translation,details,created_at FROM study_words \
         WHERE word LIKE ?1 OR translation LIKE ?1 ORDER BY id DESC LIMIT 100",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![pattern], |r| Ok(StudyWord {
        id: r.get(0)?, word: r.get(1)?, translation: r.get(2)?,
        details: r.get(3)?, created_at: r.get(4)?,
    })).map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}
