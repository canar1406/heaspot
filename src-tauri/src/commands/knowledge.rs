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

/// Reuse DNS, TLS and keep-alive connections between successive `g` queries.
/// Creating a fresh default agent for every keystroke makes the final query
/// wait behind repeated handshakes and is noticeably slower on Windows.
fn serper_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(3))
            .timeout_read(Duration::from_secs(6))
            .timeout_write(Duration::from_secs(3))
            .build()
    })
}

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
    out.replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn json_string(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn direct_question_subject(query: &str) -> Option<String> {
    let q = query
        .trim()
        .trim_matches(|c: char| matches!(c, '?' | '!' | '.' | ','))
        .trim();
    let lower = q.to_lowercase();

    for suffix in [
        " nghĩa là gì",
        " là cái gì",
        " là người nào",
        " là ở đâu",
        " là gì",
        " là ai",
        " ở đâu",
    ] {
        if lower.ends_with(suffix) {
            let subject_len = q.len().saturating_sub(suffix.len());
            let subject = q[..subject_len].trim();
            return (!subject.is_empty()).then(|| subject.to_string());
        }
    }

    for prefix in ["what is ", "who is ", "where is ", "define "] {
        if lower.starts_with(prefix) {
            let subject = q[prefix.len()..].trim();
            return (!subject.is_empty()).then(|| subject.to_string());
        }
    }
    None
}

fn plain_keyword_subject(query: &str) -> Option<String> {
    if direct_question_subject(query).is_some() {
        return None;
    }
    let subject = query
        .trim()
        .trim_matches(|c: char| matches!(c, '?' | '!' | '.' | ','))
        .trim();
    let word_count = subject.split_whitespace().count();
    (!subject.is_empty() && subject.chars().count() <= 80 && word_count <= 6)
        .then(|| subject.to_string())
}

fn fold_vietnamese_for_match(value: &str) -> String {
    value
        .chars()
        .map(|c| match c.to_lowercase().next().unwrap_or(c) {
            'à' | 'á' | 'ạ' | 'ả' | 'ã' | 'â' | 'ầ' | 'ấ' | 'ậ' | 'ẩ' | 'ẫ' | 'ă' | 'ằ' | 'ắ'
            | 'ặ' | 'ẳ' | 'ẵ' => 'a',
            'è' | 'é' | 'ẹ' | 'ẻ' | 'ẽ' | 'ê' | 'ề' | 'ế' | 'ệ' | 'ể' | 'ễ' => {
                'e'
            }
            'ì' | 'í' | 'ị' | 'ỉ' | 'ĩ' => 'i',
            'ò' | 'ó' | 'ọ' | 'ỏ' | 'õ' | 'ô' | 'ồ' | 'ố' | 'ộ' | 'ổ' | 'ỗ' | 'ơ' | 'ờ' | 'ớ'
            | 'ợ' | 'ở' | 'ỡ' => 'o',
            'ù' | 'ú' | 'ụ' | 'ủ' | 'ũ' | 'ư' | 'ừ' | 'ứ' | 'ự' | 'ử' | 'ữ' => {
                'u'
            }
            'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' => 'y',
            'đ' => 'd',
            other if other.is_alphanumeric() => other,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Các fact thuộc chính hệ sinh thái Heavietnam được trả ngay, không phụ thuộc
/// chất lượng snippet hay kết nối Google. Chỉ kích hoạt cho câu hỏi chức danh rõ ràng.
fn local_heavietnam_answer(query: &str) -> Option<QuickAnswer> {
    let folded = fold_vietnamese_for_match(query);
    let asks_person = folded
        .split_whitespace()
        .any(|token| token == "ai" || token == "who");
    let mentions_heavietnam = folded.split_whitespace().any(|token| token == "heavietnam");
    let asks_role = folded
        .split_whitespace()
        .any(|token| token == "admin" || token == "founder")
        || folded.contains("nguoi sang lap")
        || folded.contains("chu heavietnam");
    if !asks_person || !mentions_heavietnam || !asks_role {
        return None;
    }

    Some(QuickAnswer {
        answer: "Admin và founder của Heavietnam là Võ Nguyễn Hoàng Long.".to_string(),
        source: "Tri thức nội bộ Heavietnam".to_string(),
        url: "https://heavietnam.com".to_string(),
        related: vec!["HeaSpot thuộc hệ sinh thái Heavietnam.".to_string()],
        ..QuickAnswer::default()
    })
}

fn subject_appears_in(candidate: &str, subject: &str) -> bool {
    candidate.to_lowercase().contains(&subject.to_lowercase())
}

fn normalized_token(token: &str) -> String {
    token
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b_chars.len()).collect();
    for (i, a_char) in a.chars().enumerate() {
        let mut current = vec![i + 1];
        for (j, b_char) in b_chars.iter().enumerate() {
            current.push(
                (current[j] + 1)
                    .min(previous[j + 1] + 1)
                    .min(previous[j] + usize::from(a_char != *b_char)),
            );
        }
        previous = current;
    }
    previous[b_chars.len()]
}

/// Chỉ sửa typo rất nhỏ ở tên đủ dài; số model phải khớp tuyệt đối.
/// Nhờ vậy `cucktech 25` khớp `Cuktech 25`, nhưng không đổi một tên ngắn
/// hay model khác thành thực thể không liên quan.
fn token_matches(candidate: &str, subject: &str) -> bool {
    let candidate = normalized_token(candidate);
    let subject = normalized_token(subject);
    if candidate.is_empty() || subject.is_empty() {
        return false;
    }
    if candidate == subject {
        return true;
    }
    if candidate.chars().all(|c| c.is_ascii_digit()) || subject.chars().all(|c| c.is_ascii_digit())
    {
        return false;
    }
    candidate.chars().count() >= 5
        && subject.chars().count() >= 5
        && levenshtein(&candidate, &subject) <= 1
}

fn fuzzy_subject_appears_in(candidate: &str, subject: &str) -> bool {
    let candidate_tokens: Vec<&str> = candidate.split_whitespace().collect();
    let subject_tokens: Vec<&str> = subject.split_whitespace().collect();
    !subject_tokens.is_empty()
        && subject_tokens.iter().all(|subject_token| {
            candidate_tokens
                .iter()
                .any(|candidate_token| token_matches(candidate_token, subject_token))
        })
        && subject_tokens
            .iter()
            .any(|token| normalized_token(token).chars().any(|c| c.is_alphabetic()))
}

fn canonical_subject(candidate: &str, subject: &str) -> Option<String> {
    let candidate_tokens: Vec<&str> = candidate.split_whitespace().collect();
    let subject_tokens: Vec<&str> = subject.split_whitespace().collect();
    if subject_tokens.is_empty() || candidate_tokens.len() < subject_tokens.len() {
        return None;
    }
    candidate_tokens
        .windows(subject_tokens.len())
        .find(|window| {
            window
                .iter()
                .zip(&subject_tokens)
                .all(|(candidate_token, subject_token)| {
                    token_matches(candidate_token, subject_token)
                })
        })
        .map(|window| {
            window
                .iter()
                .map(|token| {
                    token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|name| !name.is_empty())
}

fn corrected_query_from_serper(value: &serde_json::Value) -> String {
    [
        value.pointer("/spellingCorrection/correctedQuery"),
        value.pointer("/searchInformation/correctedQuery"),
        value.get("correctedQuery"),
        value.get("spellingCorrection"),
    ]
    .into_iter()
    .flatten()
    .find_map(|entry| entry.as_str())
    .map(html_to_text)
    .unwrap_or_default()
}

fn ai_overview_text(value: &serde_json::Value) -> String {
    let Some(overview) = value.get("aiOverview") else {
        return String::new();
    };
    if let Some(text) = overview.as_str() {
        return html_to_text(text);
    }
    for key in ["answer", "text", "snippet"] {
        let text = html_to_text(&json_string(overview, key));
        if !text.is_empty() {
            return text;
        }
    }
    for collection in ["blocks", "items"] {
        if let Some(items) = overview.get(collection).and_then(|item| item.as_array()) {
            for item in items {
                for key in ["answer", "text", "snippet"] {
                    let text = html_to_text(&json_string(item, key));
                    if !text.is_empty() {
                        return text;
                    }
                }
            }
        }
    }
    String::new()
}

fn inferred_entity_answer(
    query: &str,
    corrected_query: &str,
    references: &[KnowledgeHit],
) -> Option<(String, String, String, String)> {
    let requested_subject = direct_question_subject(query)?;
    let corrected_subject = direct_question_subject(corrected_query)
        .or_else(|| (!corrected_query.is_empty()).then(|| corrected_query.to_string()))
        .unwrap_or_else(|| requested_subject.clone());

    let relevant: Vec<&KnowledgeHit> = references
        .iter()
        .filter(|reference| {
            let haystack = format!("{} {}", reference.title, reference.extract);
            fuzzy_subject_appears_in(&haystack, &corrected_subject)
                || fuzzy_subject_appears_in(&haystack, &requested_subject)
        })
        .collect();
    if relevant.len() < 2 {
        return None;
    }

    let display_name = relevant
        .iter()
        .find_map(|reference| canonical_subject(&reference.title, &corrected_subject))
        .or_else(|| {
            relevant
                .iter()
                .find_map(|reference| canonical_subject(&reference.extract, &corrected_subject))
        })
        .unwrap_or(corrected_subject);

    // Các alias cùng một hàng quy về một nhãn. Chỉ dùng khi >= 2 nguồn phù hợp
    // nhắc đến cùng loại thực thể để không biến một snippet lạc đề thành "đáp án".
    const ENTITY_TYPES: &[(&str, &[&str])] = &[
        (
            "pin sạc dự phòng",
            &[
                "pin sạc dự phòng",
                "sạc dự phòng",
                "pin dự phòng",
                "viên pin",
            ],
        ),
        (
            "tai nghe không dây",
            &["tai nghe không dây", "tai nghe bluetooth"],
        ),
        (
            "điện thoại thông minh",
            &["điện thoại thông minh", "smartphone"],
        ),
        ("máy tính xách tay", &["máy tính xách tay", "laptop"]),
        ("máy tính bảng", &["máy tính bảng", "tablet"]),
        ("đồng hồ thông minh", &["đồng hồ thông minh", "smartwatch"]),
        (
            "ngôn ngữ lập trình",
            &["ngôn ngữ lập trình", "programming language"],
        ),
        ("trình duyệt web", &["trình duyệt web", "web browser"]),
        ("ứng dụng", &["ứng dụng", "application", " app "]),
        ("trò chơi", &["trò chơi", "video game"]),
        ("website", &["website", "trang web"]),
        ("thương hiệu", &["thương hiệu", "brand"]),
        ("công ty", &["công ty", "company"]),
        ("tổ chức", &["tổ chức", "organization"]),
    ];

    let entity_type = ENTITY_TYPES.iter().find_map(|(label, aliases)| {
        let count = relevant
            .iter()
            .filter(|reference| {
                let text = format!(" {} {} ", reference.title, reference.extract).to_lowercase();
                aliases.iter().any(|alias| text.contains(alias))
            })
            .count();
        (count >= 2).then_some(*label)
    })?;

    let answer = format!("{display_name} là một {entity_type}.");
    let source = format!("Tổng hợp từ {} kết quả Google phù hợp", relevant.len());
    let inferred_query = query.replacen(&requested_subject, &display_name, 1);
    let effective_query = if corrected_query.is_empty() {
        inferred_query
    } else {
        corrected_query.to_string()
    };
    Some((answer, source, relevant[0].url.clone(), effective_query))
}

fn title_case_name_at_end(sentence: &str) -> Option<String> {
    let mut name_parts = Vec::new();
    for token in sentence.split_whitespace().rev().take(5) {
        let cleaned = token.trim_matches(|c: char| !c.is_alphabetic() && c != '-' && c != '_');
        let starts_uppercase = cleaned.chars().next().is_some_and(char::is_uppercase);
        if cleaned.is_empty() || !starts_uppercase {
            break;
        }
        name_parts.push(cleaned);
    }
    if name_parts.len() < 2 {
        return None;
    }
    name_parts.reverse();
    Some(name_parts.join(" "))
}

/// Bắt quan hệ chức danh nằm ở câu kế tiếp tên người, ví dụ snippet:
/// `... Võ Nguyễn Hoàng Long. Founder và Admin.`
fn extract_role_answer(
    query: &str,
    references: &[KnowledgeHit],
) -> Option<(String, String, String)> {
    let folded_query = fold_vietnamese_for_match(query);
    let role = if folded_query
        .split_whitespace()
        .any(|token| token == "admin")
    {
        "Admin"
    } else if folded_query
        .split_whitespace()
        .any(|token| token == "founder")
        || folded_query.contains("nguoi sang lap")
    {
        "Founder"
    } else {
        return None;
    };
    let organization = direct_question_subject(query)?
        .split_whitespace()
        .filter(|token| {
            let folded = fold_vietnamese_for_match(token);
            folded != "admin"
                && folded != "founder"
                && folded != "cua"
                && folded != "nguoi"
                && folded != "sang"
                && folded != "lap"
        })
        .collect::<Vec<_>>()
        .join(" ");
    if organization.is_empty() {
        return None;
    }

    for reference in references {
        let combined = format!("{}. {}", reference.title, reference.extract);
        let folded_combined = fold_vietnamese_for_match(&combined);
        if !folded_combined.contains(&fold_vietnamese_for_match(&organization))
            || !folded_combined.contains(&role.to_lowercase())
        {
            continue;
        }
        let sentences: Vec<&str> = combined.split(['.', '!', '?']).collect();
        for (index, sentence) in sentences.iter().enumerate() {
            let folded_sentence = fold_vietnamese_for_match(sentence);
            if !(folded_sentence.contains("admin") || folded_sentence.contains("founder")) {
                continue;
            }
            if let Some(name) = index
                .checked_sub(1)
                .and_then(|previous| title_case_name_at_end(sentences[previous]))
            {
                return Some((
                    format!("{role} của {organization} là {name}."),
                    reference.title.clone(),
                    reference.url.clone(),
                ));
            }
        }
    }
    None
}

fn extract_direct_sentence(query: &str, snippet: &str) -> Option<String> {
    let subject = direct_question_subject(query)
        .or_else(|| plain_keyword_subject(query))?
        .to_lowercase();
    let cleaned = html_to_text(snippet);

    for sentence in cleaned.split_inclusive(['.', '!', '?']) {
        let candidate = sentence
            .trim()
            .trim_start_matches(['-', '–', '—', '•'])
            .trim();
        let lower = candidate.to_lowercase();
        let is_definition = lower.contains(" là ")
            || lower.contains(" viết tắt của ")
            || lower.contains(" is ")
            || lower.contains(" stands for ");
        if candidate.chars().count() >= 12 && subject_appears_in(&lower, &subject) && is_definition
        {
            return Some(candidate.to_string());
        }
    }
    None
}

fn parse_serper_response(query: &str, value: &serde_json::Value) -> QuickAnswer {
    let corrected_query = corrected_query_from_serper(value);
    let mut references = Vec::new();
    if let Some(organic) = value.get("organic").and_then(|x| x.as_array()) {
        for result in organic.iter().take(5) {
            let extract = html_to_text(&json_string(result, "snippet"));
            if extract.is_empty() {
                continue;
            }
            references.push(KnowledgeHit {
                title: html_to_text(&json_string(result, "title")),
                extract,
                url: json_string(result, "link"),
            });
        }
    }

    let ai_answer = ai_overview_text(value);
    if !ai_answer.is_empty() {
        return QuickAnswer {
            answer: ai_answer,
            source: "Google AI Overview".to_string(),
            url: references
                .first()
                .map(|r| r.url.clone())
                .unwrap_or_default(),
            corrected_query,
            references,
            ..QuickAnswer::default()
        };
    }

    if let Some(answer_box) = value.get("answerBox") {
        let answer = ["answer", "snippet"]
            .iter()
            .map(|key| html_to_text(&json_string(answer_box, key)))
            .find(|answer| !answer.is_empty())
            .unwrap_or_default();
        if !answer.is_empty() {
            let title = html_to_text(&json_string(answer_box, "title"));
            let answer_url = json_string(answer_box, "link");
            return QuickAnswer {
                answer,
                source: if title.is_empty() {
                    "Google Answer Box".to_string()
                } else {
                    title
                },
                url: if answer_url.is_empty() {
                    references
                        .first()
                        .map(|r| r.url.clone())
                        .unwrap_or_default()
                } else {
                    answer_url
                },
                related: Vec::new(),
                corrected_query,
                references,
            };
        }
    }

    if let Some(graph) = value.get("knowledgeGraph") {
        let description = html_to_text(&json_string(graph, "description"));
        if !description.is_empty() {
            let title = html_to_text(&json_string(graph, "title"));
            let answer = if title.is_empty() {
                description
            } else {
                format!("{title}: {description}")
            };
            return QuickAnswer {
                answer,
                source: if title.is_empty() {
                    "Google Knowledge Graph".to_string()
                } else {
                    title
                },
                url: json_string(graph, "website"),
                related: Vec::new(),
                corrected_query,
                references,
            };
        }
    }

    for reference in &references {
        if let Some(answer) = extract_direct_sentence(query, &reference.extract) {
            return QuickAnswer {
                answer,
                source: reference.title.clone(),
                url: reference.url.clone(),
                related: Vec::new(),
                corrected_query,
                references,
            };
        }
    }

    if let Some((answer, source, url)) = extract_role_answer(query, &references) {
        return QuickAnswer {
            answer,
            source,
            url,
            corrected_query,
            references,
            ..QuickAnswer::default()
        };
    }

    if let Some((answer, source, url, corrected_query)) =
        inferred_entity_answer(query, &corrected_query, &references)
    {
        return QuickAnswer {
            answer,
            source,
            url,
            corrected_query,
            references,
            ..QuickAnswer::default()
        };
    }

    // Với một từ khóa/tên thực thể, một snippet tốt nhất là phần giới thiệu,
    // không phải "đáp án" tổng hợp. Chỉ lấy một nguồn, tuyệt đối không ghép nhiều mẩu.
    if plain_keyword_subject(query).is_some() {
        if let Some(reference) = references.first() {
            return QuickAnswer {
                answer: reference.extract.clone(),
                source: reference.title.clone(),
                url: reference.url.clone(),
                related: Vec::new(),
                corrected_query,
                references,
            };
        }
    }

    QuickAnswer {
        source: "Google (Serper)".to_string(),
        url: references
            .first()
            .map(|r| r.url.clone())
            .unwrap_or_default(),
        references,
        corrected_query,
        ..QuickAnswer::default()
    }
}

/// Serper.dev — Google SERP API (đăng ký free 2500 lượt, KHÔNG cần thẻ).
/// Chỉ trả câu trả lời trực tiếp; organic snippets được giữ riêng làm nguồn tham khảo.
fn serper_search(query: &str, key: &str) -> QuickAnswer {
    let body_json = serde_json::json!({ "q": query, "gl": "vn", "hl": "vi", "num": 5 });
    let resp = match serper_agent()
        .post("https://google.serper.dev/search")
        .set("X-API-KEY", key)
        .set("Content-Type", "application/json")
        .set("User-Agent", BROWSER_UA)
        .send_json(body_json)
    {
        Ok(r) => r.into_string().unwrap_or_default(),
        Err(_) => return QuickAnswer::default(),
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&resp) else {
        return QuickAnswer::default();
    };

    parse_serper_response(query, &v)
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
        && s.chars()
            .all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '-')
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

    out.phonetic = entry
        .get("phonetic")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
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
            let pos = m
                .get("partOfSpeech")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(defs) = m.get("definitions").and_then(|x| x.as_array()) {
                for d in defs.iter().take(2) {
                    if out.defs.len() >= 6 {
                        break;
                    }
                    let en = d
                        .get("definition")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let ex = d
                        .get("example")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
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
    let h_right = thread::spawn(move || {
        http_json(&format!(
            "https://api.datamuse.com/words?lc={w1}&sp=*&max=6"
        ))
    });
    let left = http_json(&format!(
        "https://api.datamuse.com/words?rc={word}&sp=*&max=6"
    ));
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
    out.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
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
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
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
    pub references: Vec<KnowledgeHit>,
    pub corrected_query: String,
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
    if let Some(answer) = local_heavietnam_answer(&q) {
        return Ok(answer);
    }
    let key = q.to_lowercase();
    if let Some(hit) = quick_answer_cache()
        .lock()
        .ok()
        .and_then(|c| c.get(&key).cloned())
    {
        return Ok(hit);
    }
    let serper_key = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT value FROM settings WHERE key='serper_api_key'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .filter(|k| !k.trim().is_empty())
    };
    let ans = tauri::async_runtime::spawn_blocking(move || {
        let mut serper_fallback = QuickAnswer::default();
        // Ưu tiên Serper/Google (phủ hết) nếu có key
        if let Some(key) = serper_key {
            let b = serper_search(&q, &key);
            if !b.answer.trim().is_empty() {
                return b;
            }
            serper_fallback = b;
        }
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            urlencoding(&q)
        );
        let Some(v) = http_json(&url) else {
            return serper_fallback;
        };
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        // Ưu tiên: Answer > AbstractText > Definition
        let answer = {
            let a = s("Answer");
            if !a.is_empty() {
                a
            } else {
                let ab = s("AbstractText");
                if !ab.is_empty() {
                    ab
                } else {
                    s("Definition")
                }
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

        if answer.trim().is_empty() {
            return serper_fallback;
        }

        QuickAnswer {
            answer,
            source,
            url,
            related,
            references: serper_fallback.references,
            corrected_query: serper_fallback.corrected_query,
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    if !ans.answer.trim().is_empty() {
        if let Ok(mut c) = quick_answer_cache().lock() {
            if c.len() >= 500 {
                c.clear();
            }
            c.insert(key, ans.clone());
        }
    }
    Ok(ans)
}

fn quick_answer_cache() -> &'static Mutex<HashMap<String, QuickAnswer>> {
    static C: OnceLock<Mutex<HashMap<String, QuickAnswer>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Xoá toàn bộ cache tra cứu trong RAM (dịch, wiki, quick answer).
pub fn clear_caches() {
    if let Ok(mut m) = translate_cache().lock() {
        m.clear();
    }
    if let Ok(mut m) = quick_cache().lock() {
        m.clear();
    }
    if let Ok(mut m) = wiki_cache().lock() {
        m.clear();
    }
    if let Ok(mut m) = quick_answer_cache().lock() {
        m.clear();
    }
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
    if let Some(full) = translate_cache()
        .lock()
        .ok()
        .and_then(|c| c.get(&key).cloned())
    {
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
        let detected = first
            .get(2)
            .and_then(|x| x.as_str())
            .unwrap_or("auto")
            .to_string();
        let target = if detected == "vi" {
            "en".to_string()
        } else {
            "vi".to_string()
        };
        let translation = if target == "vi" {
            gt_text(&first)
        } else {
            google_translate(&q, "auto", &target)
                .map(|v| gt_text(&v))
                .unwrap_or_default()
        };
        if translation.trim().is_empty() {
            return Err("dịch vụ dịch không trả về kết quả".to_string());
        }
        Ok::<_, String>(QuickTranslation {
            translation,
            source_language: detected,
            target_language: target,
        })
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
    if let Some(hit) = translate_cache()
        .lock()
        .ok()
        .and_then(|c| c.get(&key).cloned())
    {
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

    let first = h_trans
        .join()
        .ok()
        .flatten()
        .ok_or("dịch vụ dịch không phản hồi")?;
    let detected = first
        .get(2)
        .and_then(|x| x.as_str())
        .unwrap_or("auto")
        .to_string();
    let target = if detected == "vi" {
        "en".to_string()
    } else {
        "vi".to_string()
    };
    let translation = if target == "vi" {
        gt_text(&first)
    } else {
        google_translate(q, "auto", &target)
            .map(|v| gt_text(&v))
            .unwrap_or_default()
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
            let vi_full = google_translate(&joined, "en", "vi")
                .map(|v| gt_text(&v))
                .unwrap_or_default();
            let vi_lines: Vec<&str> = vi_full.split('\n').collect();
            for (i, (pos, en, ex)) in d.defs.iter().enumerate() {
                let vi = vi_lines
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serper_answer_box_is_the_direct_answer() {
        let value = serde_json::json!({
            "answerBox": {
                "title": "Example",
                "answer": "42",
                "link": "https://example.com/answer"
            },
            "organic": [{
                "title": "Unrelated result",
                "snippet": "This must remain a reference, not part of the answer.",
                "link": "https://example.com/result"
            }]
        });

        let result = parse_serper_response("the answer", &value);
        assert_eq!(result.answer, "42");
        assert_eq!(result.source, "Example");
        assert_eq!(result.references.len(), 1);
        assert!(!result.answer.contains("Unrelated result"));
    }

    #[test]
    fn serper_knowledge_graph_is_the_direct_answer() {
        let value = serde_json::json!({
            "knowledgeGraph": {
                "title": "Rust",
                "description": "A programming language.",
                "website": "https://www.rust-lang.org"
            }
        });

        let result = parse_serper_response("Rust là gì", &value);
        assert_eq!(result.answer, "Rust: A programming language.");
        assert_eq!(result.url, "https://www.rust-lang.org");
    }

    #[test]
    fn definition_question_extracts_one_direct_sentence_from_organic_result() {
        let value = serde_json::json!({
            "organic": [
                {
                    "title": "Installing Guide - Heavietnam",
                    "snippet": "Các hướng dẫn được tổng hợp từ nhiều nguồn có uy tín.",
                    "link": "https://heavietnam.example/install"
                },
                {
                    "title": "Welcome to Heavietnam",
                    "snippet": "Lịch sử. Heavietnam là viết tắt của Hackintosh Enthusiasts Association Viet Nam. Hội những người đam mê Hackintosh Việt Nam.",
                    "link": "https://heavietnam.example/about"
                }
            ]
        });

        let result = parse_serper_response("heavietnam là gì", &value);
        assert_eq!(
            result.answer,
            "Heavietnam là viết tắt của Hackintosh Enthusiasts Association Viet Nam."
        );
        assert_eq!(result.source, "Welcome to Heavietnam");
        assert_eq!(result.references.len(), 2);
        assert!(!result.answer.contains("Installing Guide"));
    }

    #[test]
    fn unrelated_organic_results_are_not_presented_as_an_answer() {
        let value = serde_json::json!({
            "organic": [{
                "title": "A search result",
                "snippet": "This page mentions several unrelated topics without defining the query.",
                "link": "https://example.com"
            }]
        });

        let result = parse_serper_response("heavietnam là gì", &value);
        assert!(result.answer.is_empty());
        assert_eq!(result.references.len(), 1);
    }

    #[test]
    fn short_entity_keyword_gets_a_definition_without_la_gi_suffix() {
        let value = serde_json::json!({
            "organic": [{
                "title": "Heavn",
                "snippet": "Heavn is a dating app for Christian singles seeking a serious relationship.",
                "link": "https://heavn.example/about"
            }]
        });

        let result = parse_serper_response("heavn", &value);
        assert_eq!(
            result.answer,
            "Heavn is a dating app for Christian singles seeking a serious relationship."
        );
        assert_eq!(result.source, "Heavn");
    }

    #[test]
    fn generic_keyword_uses_only_the_best_intro_snippet() {
        let value = serde_json::json!({
            "organic": [
                {
                    "title": "Primary result",
                    "snippet": "A concise introduction to the requested topic.",
                    "link": "https://example.com/primary"
                },
                {
                    "title": "Secondary result",
                    "snippet": "This second result must remain only a reference.",
                    "link": "https://example.com/secondary"
                }
            ]
        });

        let result = parse_serper_response("topic", &value);
        assert_eq!(
            result.answer,
            "A concise introduction to the requested topic."
        );
        assert!(!result.answer.contains("second result"));
        assert_eq!(result.references.len(), 2);
    }

    #[test]
    fn tiny_brand_typo_with_matching_model_gets_consensus_product_answer() {
        let value = serde_json::json!({
            "organic": [
                {
                    "title": "Đây là viên pin Cuktech 25 SE sau 2 tháng sử dụng",
                    "snippet": "Đánh giá viên pin Cuktech 25 SE và thời lượng sử dụng thực tế.",
                    "link": "https://example.com/review"
                },
                {
                    "title": "Mua Pin Sạc Dự Phòng Cuktech 25 chính hãng",
                    "snippet": "Pin sạc dự phòng Cuktech 25 có dung lượng lớn.",
                    "link": "https://example.com/store"
                },
                {
                    "title": "CUKTECH Việt Nam",
                    "snippet": "Nhà phân phối sản phẩm CUKTECH chính hãng.",
                    "link": "https://example.com/official"
                }
            ]
        });

        let result = parse_serper_response("cucktech 25 là gì", &value);
        assert_eq!(result.answer, "Cuktech 25 là một pin sạc dự phòng.");
        assert!(result.source.contains("2 kết quả Google"));
        assert_eq!(result.corrected_query, "Cuktech 25 là gì");
    }

    #[test]
    fn fuzzy_matching_never_changes_the_numeric_model() {
        assert!(fuzzy_subject_appears_in(
            "Cuktech 25 power bank",
            "cucktech 25"
        ));
        assert!(!fuzzy_subject_appears_in(
            "Cuktech 20 power bank",
            "cucktech 25"
        ));
    }

    #[test]
    fn serper_spelling_correction_is_exposed_to_the_ui() {
        let value = serde_json::json!({
            "spellingCorrection": { "correctedQuery": "cuktech 25 là gì" },
            "organic": []
        });
        let result = parse_serper_response("cucktech 25 là gì", &value);
        assert_eq!(result.corrected_query, "cuktech 25 là gì");
    }

    #[test]
    fn one_source_is_not_enough_for_a_synthesized_definition() {
        let value = serde_json::json!({
            "organic": [{
                "title": "Cuktech 25",
                "snippet": "Một bài đăng gọi đây là pin sạc dự phòng.",
                "link": "https://example.com/only"
            }]
        });
        let result = parse_serper_response("cucktech 25 là gì", &value);
        assert!(result.answer.is_empty());
    }

    #[test]
    fn heavietnam_admin_is_an_instant_local_fact() {
        for query in [
            "admin heavietnam là ai",
            "admin cua heavietnam la ai",
            "who is founder heavietnam",
            "người sáng lập heavietnam là ai",
        ] {
            let result = local_heavietnam_answer(query).expect(query);
            assert_eq!(
                result.answer,
                "Admin và founder của Heavietnam là Võ Nguyễn Hoàng Long."
            );
            assert_eq!(result.source, "Tri thức nội bộ Heavietnam");
        }
    }

    #[test]
    fn role_parser_reads_a_name_from_the_sentence_before_founder_and_admin() {
        let value = serde_json::json!({
            "organic": [{
                "title": "Welcome to Heavietnam",
                "snippet": "Heavietnam là một trang web về Hackintosh được tạo ra vào ngày 16/06. Võ Nguyễn Hoàng Long. Founder và Admin.",
                "link": "https://heavietnam.com/about"
            }]
        });
        let result = parse_serper_response("admin heavietnam là ai", &value);
        assert_eq!(
            result.answer,
            "Admin của heavietnam là Võ Nguyễn Hoàng Long."
        );
        assert_eq!(result.source, "Welcome to Heavietnam");
    }

    #[test]
    fn local_fact_does_not_hijack_unrelated_admin_queries() {
        assert!(local_heavietnam_answer("admin github là ai").is_none());
        assert!(local_heavietnam_answer("mở admin heavietnam").is_none());
    }
}
