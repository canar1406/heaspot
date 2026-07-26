//! Đổi tiền tệ + kim loại quý (vàng/bạc) theo tỉ giá LIVE.
//! Fiat: open.er-api.com (miễn phí, không cần key, ~160 tiền tệ gồm VND, NGN...).
//! Vàng/bạc: gold-api.com (USD/troy ounce). Bảng tỉ giá cache 6 giờ trong RAM.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const UA: &str = "Mozilla/5.0 (Windows NT 10.0) HeaSpot";

#[derive(Serialize, Clone)]
pub struct CurrencyResult {
    pub amount: f64,
    pub from: String,
    pub to: String,
    pub result: f64,
    /// 1 `from` = `rate` `to`
    pub rate: f64,
    /// Nguồn dữ liệu (để ghi rõ lấy từ đâu)
    pub source: String,
    /// Thời điểm tỉ giá được cập nhật (theo nguồn)
    pub updated: String,
}

#[derive(Clone)]
struct Rates {
    map: HashMap<String, f64>,
    fiat_updated: String,
}

/// Cache bảng "X trên 1 USD" + thời điểm lấy. TTL ngắn để giá cập nhật thường xuyên.
const CACHE_TTL_SECS: u64 = 20 * 60; // 20 phút

fn cache() -> &'static Mutex<Option<(Instant, Rates)>> {
    static C: OnceLock<Mutex<Option<(Instant, Rates)>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

/// Xoá cache tỉ giá (dùng cho Clear cache trong Settings).
pub fn clear_currency_cache() {
    if let Ok(mut c) = cache().lock() {
        *c = None;
    }
}

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

/// Bảng tỉ giá "số đơn vị trên 1 USD" (fiat + XAU/XAG), cache TTL ngắn.
fn get_rates() -> Result<Rates, String> {
    if let Ok(guard) = cache().lock() {
        if let Some((at, rates)) = guard.as_ref() {
            if at.elapsed() < Duration::from_secs(CACHE_TTL_SECS) {
                return Ok(rates.clone());
            }
        }
    }

    // Fiat
    let val = http_json("https://open.er-api.com/v6/latest/USD")
        .ok_or("Không lấy được tỉ giá (kiểm tra mạng)")?;
    let rates_obj = val
        .get("rates")
        .and_then(|r| r.as_object())
        .ok_or("Dữ liệu tỉ giá không hợp lệ")?;
    let fiat_updated = val
        .get("time_last_update_utc")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let mut map: HashMap<String, f64> = HashMap::new();
    for (k, v) in rates_obj {
        if let Some(n) = v.as_f64() {
            map.insert(k.to_uppercase(), n);
        }
    }

    // Vàng/bạc (best-effort): gold-api trả USD/oz -> lưu 1/price (oz trên 1 USD).
    for sym in ["XAU", "XAG"] {
        if let Some(g) = http_json(&format!("https://api.gold-api.com/price/{sym}")) {
            if let Some(p) = g.get("price").and_then(|x| x.as_f64()) {
                if p > 0.0 {
                    map.insert(sym.to_string(), 1.0 / p);
                }
            }
        }
    }
    // Đơn vị vàng Việt Nam: 1 cây (lượng) = 37.5g, 1 chỉ = 3.75g. 1 troy oz = 31.1034768g.
    // rate là "số đơn vị trên 1 USD": CAY = XAU(oz/USD) * (g mỗi oz) / (g mỗi cây).
    if let Some(&xau) = map.get("XAU") {
        const OZ_G: f64 = 31.1034768;
        map.insert("CAY".into(), xau * OZ_G / 37.5);
        map.insert("CHI".into(), xau * OZ_G / 3.75);
    }

    if map.is_empty() {
        return Err("Không có dữ liệu tỉ giá".into());
    }
    let rates = Rates { map, fiat_updated };
    if let Ok(mut guard) = cache().lock() {
        *guard = Some((Instant::now(), rates.clone()));
    }
    Ok(rates)
}

fn is_metal(code: &str) -> bool {
    matches!(code, "XAU" | "XAG" | "CAY" | "CHI")
}

fn normalize(code: &str) -> String {
    let c = code.trim().to_uppercase();
    match c.as_str() {
        "GOLD" => "XAU".into(),
        "SILVER" => "XAG".into(),
        _ => c,
    }
}

/// Đổi `amount` từ tiền `from` sang `to` (mã ISO, hoặc GOLD/SILVER).
#[tauri::command]
pub async fn currency_convert(
    amount: f64,
    from: String,
    to: String,
) -> Result<CurrencyResult, String> {
    let f = normalize(&from);
    let t = normalize(&to);
    let rates = tauri::async_runtime::spawn_blocking(get_rates)
        .await
        .map_err(|e| e.to_string())??;
    let rf = *rates
        .map
        .get(&f)
        .ok_or_else(|| format!("Không hỗ trợ: {from}"))?;
    let rt = *rates
        .map
        .get(&t)
        .ok_or_else(|| format!("Không hỗ trợ: {to}"))?;
    if rf == 0.0 {
        return Err("Tỉ giá không hợp lệ".into());
    }
    // rate là "X trên 1 USD": amount(from) -> USD = amount/rf -> to = *rt
    let rate = rt / rf;
    // Ghi rõ nguồn: fiat từ exchangerate-api.com; vàng/bạc từ gold-api.com.
    let source = if is_metal(&f) || is_metal(&t) {
        "exchangerate-api.com + gold-api.com".to_string()
    } else {
        "exchangerate-api.com (open.er-api.com)".to_string()
    };
    Ok(CurrencyResult {
        amount,
        from: f,
        to: t,
        result: amount * rate,
        rate,
        source,
        updated: rates.fiat_updated,
    })
}
