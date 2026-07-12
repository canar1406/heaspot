use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

#[derive(Serialize, Clone)]
pub struct KnowledgeHit {
    pub title: String,
    pub extract: String,
    pub url: String,
}

#[derive(Serialize, Clone)]
pub struct TranslationEntry {
    pub part_of_speech: String,
    pub definition_en: String,
    pub definition_vi: String,
    pub example: String,
}

#[derive(Serialize, Clone)]
pub struct TranslationHit {
    pub translation: String,
    pub source_language: String,
    pub target_language: String,
    pub phonetic: String,
    pub audio_url: String,
    pub collocations: Vec<String>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub entries: Vec<TranslationEntry>,
}

const UA: &str = "HeaSpot/0.1 desktop launcher";

const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// GET một URL trả JSON (native, không qua PowerShell -> nhanh hơn nhiều)
fn http_json(url: &str) -> Option<serde_json::Value> {
    let body = ureq::get(url)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(8))
        .call()
        .ok()?
        .into_string()
        .ok()?;
    serde_json::from_str(&body).ok()
}

/// Bỏ thẻ HTML + giải một số entity phổ biến.
fn html_to_text(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&#x27;", "'").replace("&#39;", "'").replace("&quot;", "\"")
        .replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", " ")
        .split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Serper.dev — Google SERP API (đăng ký free 2500 lượt, KHÔNG cần thẻ).
/// Trả kết quả Google thật: answerBox + knowledgeGraph + organic snippets -> phủ MỌI truy vấn.
fn serper_search(query: &str, key: &str) -> QuickAnswer {
    let body_json = serde_json::json!({ "q": query, "gl": "vn", "hl": "vi", "num": 5 });
    let resp = match ureq::post("https://google.serper.dev/search")
        .set("X-API-KEY", key)
        .set("Content-Type", "application/json")
        .set("User-Agent", BROWSER_UA)
        .timeout(Duration::from_secs(8))
        .send_json(body_json)
    {
        Ok(r) => r.into_string().unwrap_or_default(),
        Err(_) => return QuickAnswer::default(),
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&resp) else {
        return QuickAnswer::default();
    };

    let mut parts: Vec<String> = Vec::new();
    let mut first_url = String::new();
    let s = |val: &serde_json::Value, k: &str| val.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();

    // 1. Answer box (giống ô trả lời nhanh của Google)
    if let Some(ab) = v.get("answerBox") {
        let a = s(ab, "answer");
        let sn = s(ab, "snippet");
        let best = if !a.is_empty() { a } else { sn };
        if !best.is_empty() {
            parts.push(html_to_text(&best));
        }
    }
    // 2. Knowledge graph (thực thể: sản phẩm, người, nơi chốn…)
    if let Some(kg) = v.get("knowledgeGraph") {
        let desc = s(kg, "description");
        let title = s(kg, "title");
        if !desc.is_empty() {
            parts.push(html_to_text(&format!("{title}: {desc}")));
        }
    }
    // 3. Vài kết quả organic đầu (snippet trang web thật)
    if let Some(org) = v.get("organic").and_then(|x| x.as_array()) {
        for r in org.iter().take(4) {
            if first_url.is_empty() {
                first_url = s(r, "link");
            }
            let title = s(r, "title");
            let sn = s(r, "snippet");
            if !sn.is_empty() {
                parts.push(html_to_text(&format!("• {title}: {sn}")));
            }
            if parts.len() >= 5 {
                break;
            }
        }
    }

    QuickAnswer {
        answer: parts.join("\n\n"),
        source: "Google (Serper)".to_string(),
        url: first_url,
        related: Vec::new(),
    }
}

/// Google Translate (endpoint gtx). Trả JSON thô của translate_a/single.
fn google_translate(text: &str, sl: &str, tl: &str) -> Option<serde_json::Value> {
    let body = ureq::get("https://translate.googleapis.com/translate_a/single")
        .query("client", "gtx")
        .query("sl", sl)
        .query("tl", tl)
        .query("dt", "t")
        .query("q", text)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(8))
        .call()
        .ok()?
        .into_string()
        .ok()?;
    serde_json::from_str(&body).ok()
}

/// Ghép các đoạn dịch trong response Google ([0][*][0]) thành chuỗi.
fn gt_text(v: &serde_json::Value) -> String {
    v.get(0)
        .and_then(|a| a.as_array())
        .map(|segs| {
            segs.iter()
                .filter_map(|s| s.get(0).and_then(|x| x.as_str()))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn is_english_word(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty()
        && s.len() <= 40
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars().all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '-')
}

/// Khử trùng lặp giữ thứ tự, bỏ rỗng.
fn dedup(v: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    v.retain(|s| !s.trim().is_empty() && seen.insert(s.to_lowercase()));
}

struct DictData {
    phonetic: String,
    audio: String,
    defs: Vec<(String, String, String)>, // (part_of_speech, definition_en, example)
    synonyms: Vec<String>,
    antonyms: Vec<String>,
}

/// Gọi dictionaryapi.dev, parse phonetic/audio/definitions/synonyms.
fn fetch_dict(word: &str) -> DictData {
    let mut out = DictData {
        phonetic: String::new(),
        audio: String::new(),
        defs: Vec::new(),
        synonyms: Vec::new(),
        antonyms: Vec::new(),
    };
    let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{word}");
    let Some(v) = http_json(&url) else { return out };
    let Some(entry) = v.get(0) else { return out };

    out.phonetic = entry.get("phonetic").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if let Some(phs) = entry.get("phonetics").and_then(|x| x.as_array()) {
        for p in phs {
            if let Some(a) = p.get("audio").and_then(|x| x.as_str()) {
                if !a.is_empty() {
                    out.audio = a.to_string();
                    break;
                }
            }
        }
    }
    let push_strs = |dst: &mut Vec<String>, v: Option<&serde_json::Value>| {
        if let Some(arr) = v.and_then(|x| x.as_array()) {
            for s in arr {
                if let Some(s) = s.as_str() {
                    dst.push(s.to_string());
                }
            }
        }
    };
    if let Some(meanings) = entry.get("meanings").and_then(|x| x.as_array()) {
        for m in meanings {
            push_strs(&mut out.synonyms, m.get("synonyms"));
            push_strs(&mut out.antonyms, m.get("antonyms"));
            let pos = m.get("partOfSpeech").and_then(|x| x.as_str()).unwrap_or("").to_string();
            if let Some(defs) = m.get("definitions").and_then(|x| x.as_array()) {
                for d in defs.iter().take(2) {
                    if out.defs.len() >= 6 {
                        break;
                    }
                    let en = d.get("definition").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let ex = d.get("example").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    out.defs.push((pos.clone(), en, ex));
                    push_strs(&mut out.synonyms, d.get("synonyms"));
                    push_strs(&mut out.antonyms, d.get("antonyms"));
                }
            }
            if out.defs.len() >= 6 {
                break;
            }
        }
    }
    out
}

/// Collocations qua datamuse (từ đứng trước & sau) — 2 call chạy song song.
fn fetch_collocations(word: &str) -> Vec<String> {
    let mut out = Vec::new();
    let w1 = word.to_string();
    let h_right = thread::spawn(move || http_json(&format!("https://api.datamuse.com/words?lc={w1}&sp=*&max=6")));
    let left = http_json(&format!("https://api.datamuse.com/words?rc={word}&sp=*&max=6"));
    let right = h_right.join().ok().flatten();
    if let Some(arr) = right.as_ref().and_then(|v| v.as_array()) {
        for w in arr {
            if let Some(x) = w.get("word").and_then(|x| x.as_str()) {
                out.push(format!("{word} {x}"));
            }
        }
    }
    if let Some(arr) = left.as_ref().and_then(|v| v.as_array()) {
        for w in arr {
            if let Some(x) = w.get("word").and_then(|x| x.as_str()) {
                out.push(format!("{x} {word}"));
            }
        }
    }
    out
}

fn wiki_cache() -> &'static Mutex<HashMap<String, Vec<KnowledgeHit>>> {
    static C: OnceLock<Mutex<HashMap<String, Vec<KnowledgeHit>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// PHA 1 — nhanh: chỉ lấy tiêu đề + snippet ngắn qua list=search (không sinh extract).
/// Cho hiện kết quả tức thì rồi wikipedia_search bổ sung nội dung đầy đủ.
#[tauri::command]
pub async fn wiki_titles(query: String) -> Result<Vec<KnowledgeHit>, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let hits = tauri::async_runtime::spawn_blocking(move || {
        let url = format!(
            "https://vi.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&srlimit=5&srprop=snippet&format=json&utf8=1",
            urlencoding(&q)
        );
        let Some(v) = http_json(&url) else { return Vec::new() };
        let mut out = Vec::new();
        if let Some(arr) = v.get("query").and_then(|x| x.get("search")).and_then(|x| x.as_array()) {
            for s in arr {
                let title = s.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                // snippet có thẻ HTML <span> -> bỏ thẻ
                let snippet_raw = s.get("snippet").and_then(|x| x.as_str()).unwrap_or("");
                let snippet = strip_html(snippet_raw);
                if title.is_empty() {
                    continue;
                }
                let url = format!("https://vi.wikipedia.org/wiki/{}", urlencoding(&title.replace(' ', "_")));
                out.push(KnowledgeHit { title, extract: snippet, url });
            }
        }
        out
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(hits)
}

fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&quot;", "\"").replace("&amp;", "&").replace("&nbsp;", " ").trim().to_string()
}

/// PHA 2 — đầy đủ: extract intro hoàn chỉnh (giữ nguyên độ chi tiết). Có cache.
#[tauri::command]
pub async fn wikipedia_search(query: String) -> Result<Vec<KnowledgeHit>, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let key = q.to_lowercase();
    if let Some(hits) = wiki_cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Ok(hits);
    }
    let hits = tauri::async_runtime::spawn_blocking(move || {
        // exlimit=max -> lấy extract intro ĐẦY ĐỦ cho cả 5 kết quả (không cắt bớt).
        let url = format!(
            "https://vi.wikipedia.org/w/api.php?action=query&generator=search&gsrsearch={}&gsrlimit=5&prop=extracts%7Cinfo&exintro=1&explaintext=1&exlimit=max&inprop=url&format=json&utf8=1",
            urlencoding(&q)
        );
        let Some(v) = http_json(&url) else { return Vec::new() };
        let mut pages: Vec<(i64, KnowledgeHit)> = Vec::new();
        if let Some(obj) = v.get("query").and_then(|x| x.get("pages")).and_then(|x| x.as_object()) {
            for page in obj.values() {
                let title = page.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let extract = page.get("extract").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
                let url = page.get("fullurl").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let index = page.get("index").and_then(|x| x.as_i64()).unwrap_or(999);
                if !extract.is_empty() {
                    pages.push((index, KnowledgeHit { title, extract, url }));
                }
            }
        }
        pages.sort_by_key(|(i, _)| *i);
        pages.into_iter().map(|(_, h)| h).collect()
    })
    .await
    .map_err(|e| e.to_string())?;
    // CHỈ cache khi có kết quả — tránh cache rỗng do lỗi mạng nhất thời rồi kẹt mãi
    if !hits.is_empty() {
        if let Ok(mut c) = wiki_cache().lock() {
            if c.len() >= 300 {
                c.clear();
            }
            c.insert(key, hits.clone());
        }
    }
    Ok(hits)
}

/// Encode tối thiểu cho query string (đủ dùng cho từ khoá tìm kiếm).
fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[derive(Serialize, Clone, Default)]
pub struct QuickAnswer {
    pub answer: String,
    pub source: String,
    pub url: String,
    pub related: Vec<String>,
}

/// Kết quả nhanh cho `g`. Có Serper key -> phủ MỌI truy vấn (kết quả Google thật);
/// không có key -> DuckDuckGo Instant Answer (facts phổ biến) + frontend bù Wikipedia.
#[tauri::command]
pub async fn quick_answer(
    query: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<QuickAnswer, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(QuickAnswer::default());
    }
    let serper_key = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT value FROM settings WHERE key='serper_api_key'", [], |r| r.get::<_, String>(0))
            .ok()
            .filter(|k| !k.trim().is_empty())
    };
    let ans = tauri::async_runtime::spawn_blocking(move || {
        // Ưu tiên Serper/Google (phủ hết) nếu có key
        if let Some(key) = serper_key {
            let b = serper_search(&q, &key);
            if !b.answer.trim().is_empty() {
                return b;
            }
        }
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            urlencoding(&q)
        );
        let Some(v) = http_json(&url) else { return QuickAnswer::default() };
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        // Ưu tiên: Answer > AbstractText > Definition
        let answer = {
            let a = s("Answer");
            if !a.is_empty() { a }
            else {
                let ab = s("AbstractText");
                if !ab.is_empty() { ab } else { s("Definition") }
            }
        };
        let (source, url) = if !s("AbstractSource").is_empty() {
            (s("AbstractSource"), s("AbstractURL"))
        } else {
            (s("DefinitionSource"), s("DefinitionURL"))
        };
        // Vài chủ đề liên quan
        let mut related = Vec::new();
        if let Some(arr) = v.get("RelatedTopics").and_then(|x| x.as_array()) {
            for t in arr.iter().take(5) {
                if let Some(txt) = t.get("Text").and_then(|x| x.as_str()) {
                    if !txt.is_empty() {
                        related.push(txt.to_string());
                    }
                }
            }
        }

        QuickAnswer { answer, source, url, related }
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(ans)
}

/// Cache dịch trong RAM (theo phiên) — tra lại từ cũ là tức thì, không gọi mạng.
fn translate_cache() -> &'static Mutex<HashMap<String, TranslationHit>> {
    static C: OnceLock<Mutex<HashMap<String, TranslationHit>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Serialize, Clone)]
pub struct QuickTranslation {
    pub translation: String,
    pub source_language: String,
    pub target_language: String,
}

fn quick_cache() -> &'static Mutex<HashMap<String, QuickTranslation>> {
    static C: OnceLock<Mutex<HashMap<String, QuickTranslation>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Dịch NHANH: chỉ 1 request Google -> hiện bản dịch tức thì (~0.3s),
/// chi tiết từ điển tải sau bằng translate_lookup. Có cache riêng.
#[tauri::command]
pub async fn quick_translate(query: String) -> Result<QuickTranslation, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Err("nội dung dịch trống".into());
    }
    let key = q.to_lowercase();
    // Ưu tiên cache đầy đủ nếu đã có, nếu không thì cache nhanh
    if let Some(full) = translate_cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Ok(QuickTranslation {
            translation: full.translation,
            source_language: full.source_language,
            target_language: full.target_language,
        });
    }
    if let Some(hit) = quick_cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Ok(hit);
    }
    let hit = tauri::async_runtime::spawn_blocking(move || {
        let first = google_translate(&q, "auto", "vi").ok_or("dịch vụ dịch không phản hồi")?;
        let detected = first.get(2).and_then(|x| x.as_str()).unwrap_or("auto").to_string();
        let target = if detected == "vi" { "en".to_string() } else { "vi".to_string() };
        let translation = if target == "vi" {
            gt_text(&first)
        } else {
            google_translate(&q, "auto", &target).map(|v| gt_text(&v)).unwrap_or_default()
        };
        if translation.trim().is_empty() {
            return Err("dịch vụ dịch không trả về kết quả".to_string());
        }
        Ok::<_, String>(QuickTranslation { translation, source_language: detected, target_language: target })
    })
    .await
    .map_err(|e| e.to_string())??;
    if let Ok(mut c) = quick_cache().lock() {
        if c.len() >= 1000 {
            c.clear();
        }
        c.insert(key, hit.clone());
    }
    Ok(hit)
}

/// Smart Translate — native ureq + chạy song song translate/dictionary/datamuse.
#[tauri::command]
pub async fn translate_lookup(query: String) -> Result<TranslationHit, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Err("nội dung dịch trống".into());
    }
    let key = q.to_lowercase();
    if let Some(hit) = translate_cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Ok(hit);
    }

    let hit = tauri::async_runtime::spawn_blocking(move || do_translate(&q))
        .await
        .map_err(|e| e.to_string())??;

    if let Ok(mut c) = translate_cache().lock() {
        if c.len() >= 500 {
            c.clear();
        }
        c.insert(key, hit.clone());
    }
    Ok(hit)
}

fn do_translate(q: &str) -> Result<TranslationHit, String> {
    let en_input = is_english_word(q);

    // Fire song song: translate (luôn) + dict + datamuse (nếu input là 1 từ tiếng Anh)
    let q1 = q.to_string();
    let h_trans = thread::spawn(move || google_translate(&q1, "auto", "vi"));
    let (h_dict, h_dm) = if en_input {
        let w = q.to_string();
        let hd = thread::spawn(move || fetch_dict(&w));
        let w2 = q.to_string();
        let hm = thread::spawn(move || fetch_collocations(&w2));
        (Some(hd), Some(hm))
    } else {
        (None, None)
    };

    let first = h_trans.join().ok().flatten().ok_or("dịch vụ dịch không phản hồi")?;
    let detected = first.get(2).and_then(|x| x.as_str()).unwrap_or("auto").to_string();
    let target = if detected == "vi" { "en".to_string() } else { "vi".to_string() };
    let translation = if target == "vi" {
        gt_text(&first)
    } else {
        google_translate(q, "auto", &target).map(|v| gt_text(&v)).unwrap_or_default()
    };
    if translation.trim().is_empty() {
        return Err("dịch vụ dịch không trả về kết quả".into());
    }

    // Lấy dữ liệu từ điển: input tiếng Anh -> đã fire; input tiếng Việt -> tra từ tiếng Anh vừa dịch
    let lookup = if detected == "en" {
        q.to_string()
    } else if target == "en" {
        translation.trim().to_string()
    } else {
        String::new()
    };

    let (dict, mut collocations): (Option<DictData>, Vec<String>) = if en_input {
        (
            h_dict.and_then(|h| h.join().ok()),
            h_dm.and_then(|h| h.join().ok()).unwrap_or_default(),
        )
    } else if !lookup.is_empty() && is_english_word(&lookup) {
        let w = lookup.clone();
        let hd = thread::spawn(move || fetch_dict(&w));
        let w2 = lookup.clone();
        let hm = thread::spawn(move || fetch_collocations(&w2));
        (hd.join().ok(), hm.join().ok().unwrap_or_default())
    } else {
        (None, Vec::new())
    };

    let mut entries = Vec::new();
    let mut synonyms = Vec::new();
    let mut antonyms = Vec::new();
    let mut phonetic = String::new();
    let mut audio = String::new();

    if let Some(d) = dict {
        phonetic = d.phonetic;
        audio = d.audio;
        synonyms = d.synonyms;
        antonyms = d.antonyms;
        // Dịch tất cả định nghĩa trong 1 request (nối bằng xuống dòng)
        if !d.defs.is_empty() {
            let joined = d
                .defs
                .iter()
                .map(|(_, en, _)| en.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let vi_full = google_translate(&joined, "en", "vi").map(|v| gt_text(&v)).unwrap_or_default();
            let vi_lines: Vec<&str> = vi_full.split('\n').collect();
            for (i, (pos, en, ex)) in d.defs.iter().enumerate() {
                let vi = vi_lines.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
                entries.push(TranslationEntry {
                    part_of_speech: pos.clone(),
                    definition_en: en.clone(),
                    definition_vi: vi,
                    example: ex.clone(),
                });
            }
        }
    }

    dedup(&mut collocations);
    collocations.truncate(10);
    dedup(&mut synonyms);
    synonyms.truncate(12);
    dedup(&mut antonyms);
    antonyms.truncate(12);

    Ok(TranslationHit {
        translation,
        source_language: detected,
        target_language: target,
        phonetic,
        audio_url: audio,
        collocations,
        synonyms,
        antonyms,
        entries,
    })
}
