use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

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

/// Tra cứu Wikipedia tiếng Việt và trả phần mở đầu dạng plain text.
#[tauri::command]
pub async fn wikipedia_search(query: String) -> Result<Vec<KnowledgeHit>, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let script = r#"
try {
  $q = [uri]::EscapeDataString($env:WINSPOT_WIKI_QUERY)
  $uri = "https://vi.wikipedia.org/w/api.php?action=query&generator=search&gsrsearch=$q&gsrlimit=5&prop=extracts%7Cinfo&exintro=1&explaintext=1&inprop=url&format=json&utf8=1"
  $h = @{ 'User-Agent' = 'WinSpot/0.1 desktop launcher' }
  $r = Invoke-RestMethod -Uri $uri -Headers $h -Method GET -TimeoutSec 10
  $out = @($r.query.pages.PSObject.Properties.Value | Sort-Object index | ForEach-Object {
    [pscustomobject]@{ title = [string]$_.title; extract = [string]$_.extract; url = [string]$_.fullurl }
  })
  ConvertTo-Json -InputObject $out -Compress -Depth 4
} catch { '[]' }
"#;
    let stdout = tauri::async_runtime::spawn_blocking(move || {
        crate::commands::run_hidden_ps(script, &[("WINSPOT_WIKI_QUERY", &q)])
    })
    .await
    .map_err(|e| e.to_string())??;
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_default();
    let items = match value {
        serde_json::Value::Array(a) => a,
        obj @ serde_json::Value::Object(_) => vec![obj],
        _ => vec![],
    };
    Ok(items
        .into_iter()
        .filter_map(|v| {
            let title = v.get("title")?.as_str()?.to_string();
            let extract = v.get("extract").and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
            let url = v.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string();
            (!extract.is_empty()).then_some(KnowledgeHit { title, extract, url })
        })
        .collect())
}

/// Cache dịch trong RAM (theo phiên) — tra lại từ cũ là tức thì, không gọi mạng.
fn translate_cache() -> &'static Mutex<HashMap<String, TranslationHit>> {
    static C: OnceLock<Mutex<HashMap<String, TranslationHit>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Smart Translate: dịch tự động; với một từ tiếng Anh còn lấy phonetic, loại từ và định nghĩa.
#[tauri::command]
pub async fn translate_lookup(query: String) -> Result<TranslationHit, String> {
    let q = query.trim().to_string();
    if q.is_empty() { return Err("nội dung dịch trống".into()); }

    // 1) Cache hit -> trả ngay
    let key = q.to_lowercase();
    if let Some(hit) = translate_cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Ok(hit);
    }

    let script = r#"
$ErrorActionPreference = 'Stop'
function GT([string]$text, [string]$sl, [string]$tl) {
  $e = [uri]::EscapeDataString($text)
  $u = "https://translate.googleapis.com/translate_a/single?client=gtx&sl=$sl&tl=$tl&dt=t&q=$e"
  Invoke-RestMethod -Uri $u -Headers @{ 'User-Agent'='WinSpot/0.1 desktop launcher' } -TimeoutSec 10
}
try {
  $first = GT $env:WINSPOT_TRANSLATE_QUERY 'auto' 'vi'
  $detected = [string]$first[2]
  $target = if ($detected -eq 'vi') { 'en' } else { 'vi' }
  if ($target -eq 'vi') { $tr = $first } else { $tr = GT $env:WINSPOT_TRANSLATE_QUERY 'auto' $target }
  $translated = [string](($tr[0] | ForEach-Object { $_[0] }) -join '')
  $lookup = if ($detected -eq 'en') { $env:WINSPOT_TRANSLATE_QUERY.Trim() } elseif ($target -eq 'en') { $translated.Trim() } else { '' }
  $phonetic = ''
  $audio = ''
  $collocations = @()
  $synonyms = @()
  $antonyms = @()
  $entries = @()
  if ($lookup -match '^[A-Za-z][A-Za-z''-]*$') {
    try {
      $du = 'https://api.dictionaryapi.dev/api/v2/entries/en/' + [uri]::EscapeDataString($lookup)
      $dict = Invoke-RestMethod -Uri $du -Headers @{ 'User-Agent'='WinSpot/0.1 desktop launcher' } -TimeoutSec 8
      $phonetic = [string]$dict[0].phonetic
      $audio = [string](@($dict[0].phonetics | Where-Object { $_.audio } | Select-Object -First 1).audio)
      # Gom định nghĩa trước (chưa dịch)
      $defList = @()
      foreach ($meaning in @($dict[0].meanings)) {
        $synonyms += @($meaning.synonyms)
        $antonyms += @($meaning.antonyms)
        foreach ($def in @($meaning.definitions | Select-Object -First 2)) {
          if ($defList.Count -ge 6) { break }
          $defList += [pscustomobject]@{
            part_of_speech = [string]$meaning.partOfSpeech
            definition_en = [string]$def.definition
            example = [string]$def.example
          }
          $synonyms += @($def.synonyms)
          $antonyms += @($def.antonyms)
        }
        if ($defList.Count -ge 6) { break }
      }
      # Dịch TẤT CẢ định nghĩa trong 1 request (nối bằng xuống dòng) thay vì 6 request
      if ($defList.Count -gt 0) {
        $joined = ($defList | ForEach-Object { $_.definition_en }) -join "`n"
        $viResp = GT $joined 'en' 'vi'
        $viFull = [string](($viResp[0] | ForEach-Object { $_[0] }) -join '')
        $viLines = @($viFull -split "`n")
        for ($i = 0; $i -lt $defList.Count; $i++) {
          $vi = if ($i -lt $viLines.Count) { [string]$viLines[$i].Trim() } else { '' }
          $entries += [pscustomobject]@{
            part_of_speech = $defList[$i].part_of_speech
            definition_en = $defList[$i].definition_en
            definition_vi = $vi
            example = $defList[$i].example
          }
        }
      }
      try {
        $encoded = [uri]::EscapeDataString($lookup)
        $right = Invoke-RestMethod -Uri "https://api.datamuse.com/words?lc=$encoded&sp=*&max=6" -TimeoutSec 5
        $left = Invoke-RestMethod -Uri "https://api.datamuse.com/words?rc=$encoded&sp=*&max=6" -TimeoutSec 5
        $collocations += @($right | ForEach-Object { "$lookup $($_.word)" })
        $collocations += @($left | ForEach-Object { "$($_.word) $lookup" })
      } catch {}
    } catch {}
  }
  [pscustomobject]@{
    translation = $translated
    source_language = $detected
    target_language = $target
    phonetic = $phonetic
    audio_url = $audio
    collocations = @($collocations | Where-Object { $_ } | Select-Object -Unique -First 10)
    synonyms = @($synonyms | Where-Object { $_ } | Select-Object -Unique -First 12)
    antonyms = @($antonyms | Where-Object { $_ } | Select-Object -Unique -First 12)
    entries = @($entries)
  } | ConvertTo-Json -Compress -Depth 6
} catch { '{}' }
"#;
    let stdout = tauri::async_runtime::spawn_blocking(move || {
        crate::commands::run_hidden_ps(script, &[("WINSPOT_TRANSLATE_QUERY", &q)])
    }).await.map_err(|e| e.to_string())??;
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).map_err(|e| e.to_string())?;
    let translation = v.get("translation").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if translation.is_empty() { return Err("dịch vụ dịch không trả về kết quả".into()); }
    let entries = v.get("entries").and_then(|x| x.as_array()).cloned().unwrap_or_default()
        .into_iter().map(|e| TranslationEntry {
            part_of_speech: e.get("part_of_speech").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            definition_en: e.get("definition_en").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            definition_vi: e.get("definition_vi").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            example: e.get("example").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        }).collect();
    let strings = |key: &str| -> Vec<String> {
        v.get(key).and_then(|x| x.as_array()).cloned().unwrap_or_default()
            .into_iter().filter_map(|x| x.as_str().map(str::to_string)).collect()
    };
    let hit = TranslationHit {
        translation,
        source_language: v.get("source_language").and_then(|x| x.as_str()).unwrap_or("auto").to_string(),
        target_language: v.get("target_language").and_then(|x| x.as_str()).unwrap_or("vi").to_string(),
        phonetic: v.get("phonetic").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        audio_url: v.get("audio_url").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        collocations: strings("collocations"),
        synonyms: strings("synonyms"),
        antonyms: strings("antonyms"),
        entries,
    };
    // Lưu cache (giới hạn 500 mục để không phình RAM)
    if let Ok(mut c) = translate_cache().lock() {
        if c.len() >= 500 {
            c.clear();
        }
        c.insert(key, hit.clone());
    }
    Ok(hit)
}
