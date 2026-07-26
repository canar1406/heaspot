//! Local two-factor authenticator.
//!
//! Secrets are encrypted with Windows DPAPI for the current user. Standard
//! `otpauth://totp` / `otpauth://hotp` URIs and grouped Base32 seeds are
//! supported. OTP values are copied with the clipboard exclusion format so
//! they do not leak into Windows or HeaSpot clipboard history.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::Argon2;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

#[derive(Clone)]
struct OtpConfig {
    secret: Vec<u8>,
    otp_type: String,
    algorithm: String,
    digits: u32,
    period: u64,
    counter: u64,
    issuer: String,
    account_name: String,
    provider: String,
}

#[derive(Serialize)]
pub struct OtpPreview {
    pub code: String,
    pub remaining: u64,
    pub expires_at: u64,
    pub otp_type: String,
    pub algorithm: String,
    pub digits: u32,
    pub period: u64,
    pub issuer: String,
    pub account_name: String,
    pub provider: String,
}

#[derive(Serialize)]
pub struct OtpAccountView {
    pub id: i64,
    pub name: String,
    pub note: String,
    pub issuer: String,
    pub provider: String,
    pub otp_type: String,
    pub algorithm: String,
    pub digits: u32,
    pub period: u64,
    pub pinned: bool,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
    pub code: String,
    pub remaining: u64,
    pub expires_at: u64,
}

#[derive(Serialize)]
pub struct OtpHistoryView {
    pub id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub code: String,
    pub generated_at: u64,
    pub valid_until: u64,
    pub copied: bool,
}

#[derive(Serialize)]
pub struct OtpBackupResult {
    pub path: String,
    pub accounts: usize,
    pub history: usize,
    pub skipped: usize,
}

#[derive(Serialize)]
pub struct OtpQuickAddResult {
    pub account: OtpAccountView,
    pub created: bool,
}

#[derive(Serialize, Deserialize)]
struct BackupEnvelope {
    format: String,
    version: u32,
    kdf: String,
    cipher: String,
    salt: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Serialize, Deserialize)]
struct BackupPayload {
    exported_at: u64,
    accounts: Vec<BackupAccount>,
    history: Vec<BackupHistory>,
}

#[derive(Serialize, Deserialize)]
struct BackupAccount {
    backup_id: i64,
    name: String,
    note: String,
    issuer: String,
    secret: String,
    otp_type: String,
    algorithm: String,
    digits: u32,
    period: u64,
    counter: u64,
    pinned: bool,
    archived: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize, Deserialize)]
struct BackupHistory {
    account_id: i64,
    account_name: String,
    code: String,
    generated_at: u64,
    valid_until: u64,
    copied: bool,
}

struct StoredAccount {
    id: i64,
    name: String,
    note: String,
    issuer: String,
    secret_enc: Vec<u8>,
    otp_type: String,
    algorithm: String,
    digits: u32,
    period: u64,
    counter: u64,
    pinned: bool,
    archived: bool,
    created_at: String,
    updated_at: String,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn provider_for(issuer: &str, account: &str) -> String {
    let haystack = format!("{} {}", issuer, account).to_ascii_lowercase();
    if [
        "microsoft",
        "azure",
        "entra",
        "office 365",
        "outlook",
        "live.com",
    ]
    .iter()
    .any(|value| haystack.contains(value))
    {
        "Microsoft".into()
    } else if ["google", "gmail"]
        .iter()
        .any(|value| haystack.contains(value))
    {
        "Google".into()
    } else if issuer.trim().is_empty() {
        "OATH".into()
    } else {
        issuer.trim().to_string()
    }
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                out.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        out.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn decode_base32(input: &str) -> Result<Vec<u8>, String> {
    let mut normalized = String::with_capacity(input.len());
    for value in input.chars() {
        if value.is_ascii_whitespace() || value == '-' || value == '_' || value == '=' {
            continue;
        }
        let upper = value.to_ascii_uppercase();
        // Common OCR / formatted-copy ambiguities. Standard Base32 itself does
        // not use 0/1, so mapping them is deterministic and user-friendly.
        normalized.push(match upper {
            '0' => 'O',
            '1' => 'I',
            other => other,
        });
    }
    if normalized.len() < 8 {
        return Err("Secret 2FA quá ngắn".into());
    }

    let mut out = Vec::with_capacity(normalized.len() * 5 / 8);
    let mut buffer: u32 = 0;
    let mut bits = 0u8;
    for value in normalized.bytes() {
        let part = match value {
            b'A'..=b'Z' => value - b'A',
            b'2'..=b'7' => value - b'2' + 26,
            _ => {
                return Err(format!(
                    "Secret chứa ký tự không thuộc Base32: {}",
                    value as char
                ))
            }
        } as u32;
        buffer = (buffer << 5) | part;
        bits += 5;
        while bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    if out.len() < 5 {
        return Err("Secret 2FA không đủ dữ liệu".into());
    }
    Ok(out)
}

fn encode_base32(input: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut output = String::with_capacity((input.len() * 8).div_ceil(5));
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for &byte in input {
        buffer = (buffer << 8) | byte as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            output.push(ALPHABET[((buffer >> bits) & 0x1f) as usize] as char);
        }
    }
    if bits > 0 {
        output.push(ALPHABET[((buffer << (5 - bits)) & 0x1f) as usize] as char);
    }
    output
}

fn parse_otp_input(input: &str) -> Result<OtpConfig, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Hãy nhập secret Base32 hoặc URI otpauth://".into());
    }

    if input.to_ascii_lowercase().starts_with("otpauth://") {
        let rest = &input[10..];
        let (otp_type, remainder) = rest
            .split_once('/')
            .ok_or("URI otpauth thiếu loại TOTP/HOTP")?;
        let otp_type = otp_type.to_ascii_lowercase();
        if otp_type != "totp" && otp_type != "hotp" {
            return Err("HeaSpot hỗ trợ URI otpauth TOTP và HOTP".into());
        }
        let (label_raw, query) = remainder
            .split_once('?')
            .ok_or("URI otpauth thiếu tham số secret")?;
        let label = percent_decode(label_raw);
        let mut values = HashMap::new();
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            values.insert(key.to_ascii_lowercase(), percent_decode(value));
        }
        let secret_text = values.get("secret").ok_or("URI otpauth không có secret")?;
        let secret = decode_base32(secret_text)?;
        let algorithm = values
            .get("algorithm")
            .map(|value| value.replace('-', "").to_ascii_uppercase())
            .unwrap_or_else(|| "SHA1".into());
        if !matches!(algorithm.as_str(), "SHA1" | "SHA256" | "SHA512") {
            return Err(format!("Thuật toán OTP chưa hỗ trợ: {algorithm}"));
        }
        let digits = values
            .get("digits")
            .and_then(|value| value.parse().ok())
            .unwrap_or(6);
        if !(6..=8).contains(&digits) {
            return Err("OTP chỉ hỗ trợ 6–8 chữ số".into());
        }
        let period = values
            .get("period")
            .and_then(|value| value.parse().ok())
            .unwrap_or(30);
        if !(15..=300).contains(&period) {
            return Err("Chu kỳ TOTP phải từ 15 đến 300 giây".into());
        }
        let counter = values
            .get("counter")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        if otp_type == "hotp" && !values.contains_key("counter") {
            return Err("URI HOTP phải có counter".into());
        }
        let (label_issuer, account_name) = label
            .split_once(':')
            .map(|(issuer, account)| (issuer.trim(), account.trim()))
            .unwrap_or(("", label.trim()));
        let issuer = values
            .get("issuer")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(label_issuer)
            .to_string();
        let account_name = if account_name.is_empty() {
            issuer.clone()
        } else {
            account_name.to_string()
        };
        let provider = provider_for(&issuer, &account_name);
        return Ok(OtpConfig {
            secret,
            otp_type,
            algorithm,
            digits,
            period,
            counter,
            issuer,
            account_name,
            provider,
        });
    }

    // Manual setup pages often prefix the grouped seed with "Secret:" or
    // "Setup key=". Use the value after that delimiter when present.
    let raw = input
        .rsplit_once([':', '='])
        .map(|(_, value)| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(input);
    let secret = decode_base32(raw)?;
    Ok(OtpConfig {
        secret,
        otp_type: "totp".into(),
        algorithm: "SHA1".into(),
        digits: 6,
        period: 30,
        counter: 0,
        issuer: String::new(),
        account_name: String::new(),
        provider: "OATH".into(),
    })
}

fn hmac_digest(secret: &[u8], counter: u64, algorithm: &str) -> Result<Vec<u8>, String> {
    let message = counter.to_be_bytes();
    match algorithm {
        "SHA1" => {
            let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(secret)
                .map_err(|_| "Secret SHA1 không hợp lệ")?;
            mac.update(&message);
            Ok(mac.finalize().into_bytes().to_vec())
        }
        "SHA256" => {
            let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret)
                .map_err(|_| "Secret SHA256 không hợp lệ")?;
            mac.update(&message);
            Ok(mac.finalize().into_bytes().to_vec())
        }
        "SHA512" => {
            let mut mac = <Hmac<Sha512> as Mac>::new_from_slice(secret)
                .map_err(|_| "Secret SHA512 không hợp lệ")?;
            mac.update(&message);
            Ok(mac.finalize().into_bytes().to_vec())
        }
        _ => Err(format!("Thuật toán OTP chưa hỗ trợ: {algorithm}")),
    }
}

fn generate_code(
    secret: &[u8],
    counter: u64,
    algorithm: &str,
    digits: u32,
) -> Result<String, String> {
    let digest = hmac_digest(secret, counter, algorithm)?;
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    if offset + 4 > digest.len() {
        return Err("Không tạo được mã OTP".into());
    }
    let binary = ((digest[offset] as u32 & 0x7f) << 24)
        | ((digest[offset + 1] as u32) << 16)
        | ((digest[offset + 2] as u32) << 8)
        | digest[offset + 3] as u32;
    let modulus = 10u64.pow(digits);
    Ok(format!(
        "{:0width$}",
        binary as u64 % modulus,
        width = digits as usize
    ))
}

fn code_for(config: &OtpConfig, now: u64) -> Result<OtpPreview, String> {
    let (counter, remaining, expires_at) = if config.otp_type == "hotp" {
        (config.counter, 0, 0)
    } else {
        let period = config.period.max(1);
        let remaining = period - (now % period);
        (now / period, remaining, now + remaining)
    };
    Ok(OtpPreview {
        code: generate_code(&config.secret, counter, &config.algorithm, config.digits)?,
        remaining,
        expires_at,
        otp_type: config.otp_type.clone(),
        algorithm: config.algorithm.clone(),
        digits: config.digits,
        period: config.period,
        issuer: config.issuer.clone(),
        account_name: config.account_name.clone(),
        provider: config.provider.clone(),
    })
}

unsafe fn dpapi_encrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = CryptProtectData(
        &input,
        std::ptr::null(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        CRYPTPROTECT_UI_FORBIDDEN,
        &mut output,
    );
    if ok == 0 || output.pbData.is_null() {
        let code = windows_sys::Win32::Foundation::GetLastError();
        return Err(format!(
            "Windows không mã hóa được secret 2FA (Win32 error {code})"
        ));
    }
    let protected = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
    windows_sys::Win32::Foundation::LocalFree(output.pbData as _);
    Ok(protected)
}

unsafe fn dpapi_decrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = CryptUnprotectData(
        &input,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        CRYPTPROTECT_UI_FORBIDDEN,
        &mut output,
    );
    if ok == 0 || output.pbData.is_null() {
        let code = windows_sys::Win32::Foundation::GetLastError();
        return Err(format!(
            "Không giải mã được secret 2FA cho tài khoản Windows hiện tại (Win32 error {code})"
        ));
    }
    let plain = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
    windows_sys::Win32::Foundation::LocalFree(output.pbData as _);
    Ok(plain)
}

fn account_config(account: &StoredAccount) -> Result<OtpConfig, String> {
    let secret = unsafe { dpapi_decrypt(&account.secret_enc)? };
    Ok(OtpConfig {
        secret,
        otp_type: account.otp_type.clone(),
        algorithm: account.algorithm.clone(),
        digits: account.digits,
        period: account.period,
        counter: account.counter,
        issuer: account.issuer.clone(),
        account_name: account.name.clone(),
        provider: provider_for(&account.issuer, &account.name),
    })
}

fn account_view(account: StoredAccount, now: u64) -> Result<OtpAccountView, String> {
    let config = account_config(&account)?;
    let preview = code_for(&config, now)?;
    Ok(OtpAccountView {
        id: account.id,
        name: account.name,
        note: account.note,
        issuer: account.issuer,
        provider: preview.provider,
        otp_type: account.otp_type,
        algorithm: account.algorithm,
        digits: account.digits,
        period: account.period,
        pinned: account.pinned,
        archived: account.archived,
        created_at: account.created_at,
        updated_at: account.updated_at,
        code: preview.code,
        remaining: preview.remaining,
        expires_at: preview.expires_at,
    })
}

fn load_account(conn: &rusqlite::Connection, id: i64) -> Result<StoredAccount, String> {
    conn.query_row(
        "SELECT id,name,note,issuer,secret_enc,otp_type,algorithm,digits,period,counter,is_pinned,is_archived,created_at,updated_at
         FROM otp_accounts WHERE id=?1",
        params![id],
        |row| {
            Ok(StoredAccount {
                id: row.get(0)?,
                name: row.get(1)?,
                note: row.get(2)?,
                issuer: row.get(3)?,
                secret_enc: row.get(4)?,
                otp_type: row.get(5)?,
                algorithm: row.get(6)?,
                digits: row.get::<_, i64>(7)? as u32,
                period: row.get::<_, i64>(8)? as u64,
                counter: row.get::<_, i64>(9)? as u64,
                pinned: row.get::<_, i64>(10)? != 0,
                archived: row.get::<_, i64>(11)? != 0,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        },
    )
    .optional()
    .map_err(|error| error.to_string())?
    .ok_or_else(|| "Không tìm thấy tài khoản 2FA".into())
}

#[tauri::command]
pub fn preview_otp(input: String) -> Result<OtpPreview, String> {
    let config = parse_otp_input(&input)?;
    code_for(&config, unix_now())
}

#[tauri::command]
pub fn add_otp_account(
    state: State<'_, crate::AppState>,
    name: String,
    note: String,
    input: String,
) -> Result<OtpAccountView, String> {
    let mut config = parse_otp_input(&input)?;
    let final_name = if name.trim().is_empty() {
        if !config.account_name.trim().is_empty() {
            config.account_name.trim().to_string()
        } else if !config.issuer.trim().is_empty() {
            config.issuer.trim().to_string()
        } else {
            "Tài khoản 2FA".into()
        }
    } else {
        name.trim().to_string()
    };
    let secret_enc = unsafe { dpapi_encrypt(&config.secret)? };
    config.secret.fill(0);
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO otp_accounts(name,note,issuer,secret_enc,otp_type,algorithm,digits,period,counter)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            final_name,
            note.trim(),
            config.issuer,
            secret_enc,
            config.otp_type,
            config.algorithm,
            config.digits as i64,
            config.period as i64,
            config.counter as i64,
        ],
    )
    .map_err(|error| error.to_string())?;
    let account = load_account(&conn, conn.last_insert_rowid())?;
    account_view(account, unix_now())
}

#[tauri::command]
pub fn quick_add_otp_account(
    state: State<'_, crate::AppState>,
    input: String,
) -> Result<OtpQuickAddResult, String> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    quick_add_otp_account_with_conn(&conn, input)
}

fn quick_add_otp_account_with_conn(
    conn: &rusqlite::Connection,
    input: String,
) -> Result<OtpQuickAddResult, String> {
    let mut config = parse_otp_input(input.trim().trim_start_matches('+').trim())?;
    let mut statement = conn
        .prepare(
            "SELECT id,name,note,issuer,secret_enc,otp_type,algorithm,digits,period,counter,is_pinned,is_archived,created_at,updated_at
             FROM otp_accounts ORDER BY id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(StoredAccount {
                id: row.get(0)?,
                name: row.get(1)?,
                note: row.get(2)?,
                issuer: row.get(3)?,
                secret_enc: row.get(4)?,
                otp_type: row.get(5)?,
                algorithm: row.get(6)?,
                digits: row.get::<_, i64>(7)? as u32,
                period: row.get::<_, i64>(8)? as u64,
                counter: row.get::<_, i64>(9)? as u64,
                pinned: row.get::<_, i64>(10)? != 0,
                archived: row.get::<_, i64>(11)? != 0,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })
        .map_err(|error| error.to_string())?;
    let mut duplicate = None;
    for row in rows {
        let account = row.map_err(|error| error.to_string())?;
        let mut existing = unsafe { dpapi_decrypt(&account.secret_enc)? };
        let matches = existing == config.secret && account.otp_type == config.otp_type;
        existing.fill(0);
        if matches {
            duplicate = Some(account);
            break;
        }
    }
    drop(statement);
    if let Some(mut account) = duplicate {
        if account.archived {
            conn.execute(
                "UPDATE otp_accounts SET is_archived=0,updated_at=datetime('now','localtime') WHERE id=?1",
                params![account.id],
            )
            .map_err(|error| error.to_string())?;
            account.archived = false;
        }
        config.secret.fill(0);
        return Ok(OtpQuickAddResult {
            account: account_view(account, unix_now())?,
            created: false,
        });
    }

    let final_name = if !config.account_name.trim().is_empty() {
        config.account_name.trim().to_string()
    } else if !config.issuer.trim().is_empty() {
        config.issuer.trim().to_string()
    } else {
        "Tài khoản 2FA".into()
    };
    let secret_enc = unsafe { dpapi_encrypt(&config.secret)? };
    config.secret.fill(0);
    conn.execute(
        "INSERT INTO otp_accounts(name,note,issuer,secret_enc,otp_type,algorithm,digits,period,counter)
         VALUES(?1,'Thêm nhanh bằng otp +',?2,?3,?4,?5,?6,?7,?8)",
        params![
            final_name,
            config.issuer,
            secret_enc,
            config.otp_type,
            config.algorithm,
            config.digits as i64,
            config.period as i64,
            config.counter as i64,
        ],
    )
    .map_err(|error| error.to_string())?;
    let account = load_account(conn, conn.last_insert_rowid())?;
    Ok(OtpQuickAddResult {
        account: account_view(account, unix_now())?,
        created: true,
    })
}

#[tauri::command]
pub fn list_otp_accounts(
    state: State<'_, crate::AppState>,
    archived: bool,
) -> Result<Vec<OtpAccountView>, String> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    list_otp_accounts_with_conn(&conn, archived)
}

fn list_otp_accounts_with_conn(
    conn: &rusqlite::Connection,
    archived: bool,
) -> Result<Vec<OtpAccountView>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id,name,note,issuer,secret_enc,otp_type,algorithm,digits,period,counter,is_pinned,is_archived,created_at,updated_at
             FROM otp_accounts WHERE is_archived=?1
             ORDER BY created_at DESC, id DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![archived as i64], |row| {
            Ok(StoredAccount {
                id: row.get(0)?,
                name: row.get(1)?,
                note: row.get(2)?,
                issuer: row.get(3)?,
                secret_enc: row.get(4)?,
                otp_type: row.get(5)?,
                algorithm: row.get(6)?,
                digits: row.get::<_, i64>(7)? as u32,
                period: row.get::<_, i64>(8)? as u64,
                counter: row.get::<_, i64>(9)? as u64,
                pinned: row.get::<_, i64>(10)? != 0,
                archived: row.get::<_, i64>(11)? != 0,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })
        .map_err(|error| error.to_string())?;
    let now = unix_now();
    rows.map(|row| {
        row.map_err(|error| error.to_string())
            .and_then(|account| account_view(account, now))
    })
    .collect()
}

#[tauri::command]
pub fn rename_otp_account(
    state: State<'_, crate::AppState>,
    id: i64,
    name: String,
    note: String,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Tên gợi nhớ không được để trống".into());
    }
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "UPDATE otp_accounts SET name=?1,note=?2,updated_at=datetime('now','localtime') WHERE id=?3",
        params![name, note.trim(), id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn toggle_otp_pin(state: State<'_, crate::AppState>, id: i64) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let pinned: bool = conn
        .query_row(
            "SELECT is_pinned FROM otp_accounts WHERE id=?1",
            params![id],
            |row| Ok(row.get::<_, i64>(0)? != 0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or("Không tìm thấy tài khoản 2FA")?;
    conn.execute(
        "UPDATE otp_accounts SET is_pinned=?1,updated_at=datetime('now','localtime') WHERE id=?2",
        params![(!pinned) as i64, id],
    )
    .map_err(|error| error.to_string())?;
    Ok(!pinned)
}

#[tauri::command]
pub fn set_otp_archived(
    state: State<'_, crate::AppState>,
    id: i64,
    archived: bool,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    conn.execute(
        "UPDATE otp_accounts SET is_archived=?1,updated_at=datetime('now','localtime') WHERE id=?2",
        params![archived as i64, id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_otp_account(state: State<'_, crate::AppState>, id: i64) -> Result<(), String> {
    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    let transaction = conn.transaction().map_err(|error| error.to_string())?;
    transaction
        .execute("DELETE FROM otp_history WHERE account_id=?1", params![id])
        .map_err(|error| error.to_string())?;
    transaction
        .execute("DELETE FROM otp_accounts WHERE id=?1", params![id])
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn copy_otp_code(state: State<'_, crate::AppState>, id: i64) -> Result<OtpPreview, String> {
    let now = unix_now();
    let preview = {
        let conn = state.db.lock().map_err(|error| error.to_string())?;
        let account = load_account(&conn, id)?;
        let config = account_config(&account)?;
        let preview = code_for(&config, now)?;
        let code_enc = unsafe { dpapi_encrypt(preview.code.as_bytes())? };
        conn.execute(
            "INSERT INTO otp_history(account_id,account_name,code_enc,generated_at,valid_until,copied)
             VALUES(?1,?2,?3,?4,?5,1)",
            params![id, account.name, code_enc, now as i64, preview.expires_at as i64],
        )
        .map_err(|error| error.to_string())?;
        if account.otp_type == "hotp" {
            conn.execute(
                "UPDATE otp_accounts SET counter=counter+1,updated_at=datetime('now','localtime') WHERE id=?1",
                params![id],
            )
            .map_err(|error| error.to_string())?;
        }
        preview
    };
    crate::plugins::passwords::copy_secret(preview.code.clone())?;
    Ok(preview)
}

fn account_secret(conn: &rusqlite::Connection, id: i64) -> Result<String, String> {
    let account = load_account(conn, id)?;
    let mut secret = unsafe { dpapi_decrypt(&account.secret_enc)? };
    let encoded = encode_base32(&secret);
    secret.fill(0);
    Ok(encoded)
}

/// Decrypt one explicitly requested secret for display. Account listings never
/// include secret material, so merely opening the OTP screen does not expose
/// the whole vault to the WebView.
#[tauri::command]
pub fn get_otp_secret(state: State<'_, crate::AppState>, id: i64) -> Result<String, String> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    account_secret(&conn, id)
}

/// Copy a stored Base32 seed using the protected clipboard path shared with
/// passwords and OTP codes (excluded from Windows/HeaSpot history).
#[tauri::command]
pub fn copy_otp_secret(state: State<'_, crate::AppState>, id: i64) -> Result<(), String> {
    let secret = {
        let conn = state.db.lock().map_err(|error| error.to_string())?;
        account_secret(&conn, id)?
    };
    crate::plugins::passwords::copy_secret(secret)
}

#[tauri::command]
pub fn list_otp_history(
    state: State<'_, crate::AppState>,
    limit: Option<u32>,
) -> Result<Vec<OtpHistoryView>, String> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    let mut statement = conn
        .prepare(
            "SELECT id,account_id,account_name,code_enc,generated_at,valid_until,copied
             FROM otp_history ORDER BY generated_at DESC,id DESC LIMIT ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![limit.unwrap_or(100).clamp(1, 500)], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, i64>(4)? as u64,
                row.get::<_, i64>(5)? as u64,
                row.get::<_, i64>(6)? != 0,
            ))
        })
        .map_err(|error| error.to_string())?;
    rows.map(|row| {
        let (id, account_id, account_name, code_enc, generated_at, valid_until, copied) =
            row.map_err(|error| error.to_string())?;
        let code = String::from_utf8(unsafe { dpapi_decrypt(&code_enc)? })
            .map_err(|_| "Lịch sử OTP bị hỏng".to_string())?;
        Ok(OtpHistoryView {
            id,
            account_id,
            account_name,
            code,
            generated_at,
            valid_until,
            copied,
        })
    })
    .collect()
}

#[tauri::command]
pub fn clear_otp_history(state: State<'_, crate::AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM otp_history", [])
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn derive_backup_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    if password.chars().count() < 8 {
        return Err("Mật khẩu backup phải có ít nhất 8 ký tự".into());
    }
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| "Không tạo được khóa mã hóa backup".to_string())?;
    Ok(key)
}

fn normalized_backup_path(path: &str) -> Result<PathBuf, String> {
    let mut path = PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        return Err("Chưa chọn nơi lưu file backup".into());
    }
    if path.extension().is_none() {
        path.set_extension("heaspot2fa");
    }
    Ok(path)
}

#[tauri::command]
pub fn export_otp_backup(
    state: State<'_, crate::AppState>,
    path: String,
    password: String,
) -> Result<OtpBackupResult, String> {
    let conn = state.db.lock().map_err(|error| error.to_string())?;
    export_otp_backup_with_conn(&conn, path, password)
}

fn export_otp_backup_with_conn(
    conn: &rusqlite::Connection,
    path: String,
    password: String,
) -> Result<OtpBackupResult, String> {
    let path = normalized_backup_path(&path)?;

    let mut account_statement = conn
        .prepare(
            "SELECT id,name,note,issuer,secret_enc,otp_type,algorithm,digits,period,counter,is_pinned,is_archived,created_at,updated_at
             FROM otp_accounts ORDER BY id",
        )
        .map_err(|error| error.to_string())?;
    let account_rows = account_statement
        .query_map([], |row| {
            Ok(StoredAccount {
                id: row.get(0)?,
                name: row.get(1)?,
                note: row.get(2)?,
                issuer: row.get(3)?,
                secret_enc: row.get(4)?,
                otp_type: row.get(5)?,
                algorithm: row.get(6)?,
                digits: row.get::<_, i64>(7)? as u32,
                period: row.get::<_, i64>(8)? as u64,
                counter: row.get::<_, i64>(9)? as u64,
                pinned: row.get::<_, i64>(10)? != 0,
                archived: row.get::<_, i64>(11)? != 0,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })
        .map_err(|error| error.to_string())?;
    let mut accounts = Vec::new();
    for row in account_rows {
        let account = row.map_err(|error| error.to_string())?;
        let mut secret = unsafe { dpapi_decrypt(&account.secret_enc)? };
        accounts.push(BackupAccount {
            backup_id: account.id,
            name: account.name,
            note: account.note,
            issuer: account.issuer,
            secret: BASE64.encode(&secret),
            otp_type: account.otp_type,
            algorithm: account.algorithm,
            digits: account.digits,
            period: account.period,
            counter: account.counter,
            pinned: account.pinned,
            archived: account.archived,
            created_at: account.created_at,
            updated_at: account.updated_at,
        });
        secret.fill(0);
    }
    drop(account_statement);

    let mut history_statement = conn
        .prepare(
            "SELECT account_id,account_name,code_enc,generated_at,valid_until,copied
             FROM otp_history ORDER BY id",
        )
        .map_err(|error| error.to_string())?;
    let history_rows = history_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, i64>(3)? as u64,
                row.get::<_, i64>(4)? as u64,
                row.get::<_, i64>(5)? != 0,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut history = Vec::new();
    for row in history_rows {
        let (account_id, account_name, code_enc, generated_at, valid_until, copied) =
            row.map_err(|error| error.to_string())?;
        let mut code_bytes = unsafe { dpapi_decrypt(&code_enc)? };
        let code =
            String::from_utf8(code_bytes.clone()).map_err(|_| "Lịch sử OTP bị hỏng".to_string())?;
        code_bytes.fill(0);
        history.push(BackupHistory {
            account_id,
            account_name,
            code,
            generated_at,
            valid_until,
            copied,
        });
    }
    drop(history_statement);

    let account_count = accounts.len();
    let history_count = history.len();
    let payload = BackupPayload {
        exported_at: unix_now(),
        accounts,
        history,
    };
    let mut plaintext = serde_json::to_vec(&payload).map_err(|error| error.to_string())?;
    let mut salt = [0u8; 16];
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);
    let mut key = derive_backup_key(&password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| "Không khởi tạo được mã hóa backup".to_string())?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_ref())
        .map_err(|_| "Không mã hóa được backup 2FA".to_string())?;
    key.fill(0);
    plaintext.fill(0);

    let envelope = BackupEnvelope {
        format: "HeaSpot 2FA Backup".into(),
        version: 1,
        kdf: "Argon2id".into(),
        cipher: "AES-256-GCM".into(),
        salt: BASE64.encode(salt),
        nonce: BASE64.encode(nonce_bytes),
        ciphertext: BASE64.encode(ciphertext),
    };
    let output = serde_json::to_vec_pretty(&envelope).map_err(|error| error.to_string())?;
    std::fs::write(&path, output)
        .map_err(|error| format!("Không ghi được file backup: {error}"))?;

    Ok(OtpBackupResult {
        path: path.to_string_lossy().into_owned(),
        accounts: account_count,
        history: history_count,
        skipped: 0,
    })
}

#[tauri::command]
pub fn import_otp_backup(
    state: State<'_, crate::AppState>,
    path: String,
    password: String,
) -> Result<OtpBackupResult, String> {
    let mut conn = state.db.lock().map_err(|error| error.to_string())?;
    import_otp_backup_with_conn(&mut conn, path, password)
}

fn import_otp_backup_with_conn(
    conn: &mut rusqlite::Connection,
    path: String,
    password: String,
) -> Result<OtpBackupResult, String> {
    let path = PathBuf::from(path.trim());
    let metadata =
        std::fs::metadata(&path).map_err(|error| format!("Không đọc được file backup: {error}"))?;
    if metadata.len() > 25 * 1024 * 1024 {
        return Err("File backup lớn bất thường (giới hạn 25 MB)".into());
    }
    let input =
        std::fs::read(&path).map_err(|error| format!("Không đọc được file backup: {error}"))?;
    let envelope: BackupEnvelope = serde_json::from_slice(&input)
        .map_err(|_| "File không phải backup 2FA của HeaSpot".to_string())?;
    if envelope.format != "HeaSpot 2FA Backup"
        || envelope.version != 1
        || envelope.kdf != "Argon2id"
        || envelope.cipher != "AES-256-GCM"
    {
        return Err("Phiên bản hoặc định dạng backup chưa được hỗ trợ".into());
    }
    let salt = BASE64
        .decode(envelope.salt)
        .map_err(|_| "Salt của backup bị hỏng".to_string())?;
    let nonce = BASE64
        .decode(envelope.nonce)
        .map_err(|_| "Nonce của backup bị hỏng".to_string())?;
    let ciphertext = BASE64
        .decode(envelope.ciphertext)
        .map_err(|_| "Nội dung backup bị hỏng".to_string())?;
    if salt.len() != 16 || nonce.len() != 12 {
        return Err("Thông số mã hóa backup không hợp lệ".into());
    }
    let mut key = derive_backup_key(&password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| "Không khởi tạo được giải mã backup".to_string())?;
    let mut plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| "Sai mật khẩu hoặc file backup đã bị sửa/hỏng".to_string())?;
    key.fill(0);
    let payload: BackupPayload = serde_json::from_slice(&plaintext)
        .map_err(|_| "Nội dung backup không hợp lệ".to_string())?;
    plaintext.fill(0);
    if payload.accounts.len() > 10_000 || payload.history.len() > 100_000 {
        return Err("Backup chứa quá nhiều dữ liệu".into());
    }

    let mut existing_statement = conn
        .prepare("SELECT id,name,issuer,secret_enc,otp_type FROM otp_accounts ORDER BY id")
        .map_err(|error| error.to_string())?;
    let rows = existing_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut existing = Vec::new();
    for row in rows {
        let (id, name, issuer, encrypted, otp_type) = row.map_err(|error| error.to_string())?;
        existing.push((id, name, issuer, otp_type, unsafe {
            dpapi_decrypt(&encrypted)?
        }));
    }
    drop(existing_statement);

    let transaction = conn.transaction().map_err(|error| error.to_string())?;
    let mut id_map: HashMap<i64, (i64, bool)> = HashMap::new();
    let mut imported_accounts = 0usize;
    let mut skipped = 0usize;
    for account in payload.accounts {
        if account.name.trim().is_empty()
            || !matches!(account.otp_type.as_str(), "totp" | "hotp")
            || !matches!(account.algorithm.as_str(), "SHA1" | "SHA256" | "SHA512")
            || !(6..=8).contains(&account.digits)
            || !(15..=300).contains(&account.period)
        {
            return Err("Backup chứa tài khoản OTP có cấu hình không hợp lệ".into());
        }
        let mut secret = BASE64
            .decode(&account.secret)
            .map_err(|_| "Secret trong backup bị hỏng".to_string())?;
        if secret.len() < 5 {
            secret.fill(0);
            return Err("Secret trong backup quá ngắn".into());
        }
        if let Some((id, ..)) = existing
            .iter()
            .find(|(_, name, issuer, otp_type, current)| {
                name == &account.name
                    && issuer == &account.issuer
                    && otp_type == &account.otp_type
                    && current.as_slice() == secret.as_slice()
            })
        {
            id_map.insert(account.backup_id, (*id, false));
            secret.fill(0);
            skipped += 1;
            continue;
        }
        let secret_enc = unsafe { dpapi_encrypt(&secret)? };
        transaction
            .execute(
                "INSERT INTO otp_accounts(name,note,issuer,secret_enc,otp_type,algorithm,digits,period,counter,is_pinned,is_archived,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![
                    account.name,
                    account.note,
                    account.issuer,
                    secret_enc,
                    account.otp_type,
                    account.algorithm,
                    account.digits as i64,
                    account.period as i64,
                    account.counter as i64,
                    account.pinned as i64,
                    account.archived as i64,
                    account.created_at,
                    account.updated_at,
                ],
            )
            .map_err(|error| error.to_string())?;
        let new_id = transaction.last_insert_rowid();
        existing.push((
            new_id,
            account.name,
            account.issuer,
            account.otp_type,
            secret,
        ));
        id_map.insert(account.backup_id, (new_id, true));
        imported_accounts += 1;
    }

    let mut imported_history = 0usize;
    for entry in payload.history {
        let Some(&(account_id, account_was_imported)) = id_map.get(&entry.account_id) else {
            continue;
        };
        // Nếu tài khoản đã tồn tại thì history của lần import trước cũng có thể
        // đã tồn tại; bỏ qua cả nhóm để import lại cùng file không nhân bản log.
        if !account_was_imported
            || entry.code.len() > 10
            || !entry.code.chars().all(|value| value.is_ascii_digit())
        {
            continue;
        }
        let code_enc = unsafe { dpapi_encrypt(entry.code.as_bytes())? };
        transaction
            .execute(
                "INSERT INTO otp_history(account_id,account_name,code_enc,generated_at,valid_until,copied)
                 VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    account_id,
                    entry.account_name,
                    code_enc,
                    entry.generated_at as i64,
                    entry.valid_until as i64,
                    entry.copied as i64,
                ],
            )
            .map_err(|error| error.to_string())?;
        imported_history += 1;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    for (_, _, _, _, secret) in &mut existing {
        secret.fill(0);
    }

    Ok(OtpBackupResult {
        path: path.to_string_lossy().into_owned(),
        accounts: imported_accounts,
        history: imported_history,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        account_secret, code_for, dpapi_decrypt, dpapi_encrypt, encode_base32,
        export_otp_backup_with_conn, generate_code, import_otp_backup_with_conn,
        list_otp_accounts_with_conn, parse_otp_input, quick_add_otp_account_with_conn,
    };
    use rusqlite::params;

    const RFC_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    #[test]
    fn base32_roundtrip_keeps_the_original_secret() {
        let decoded = parse_otp_input(RFC_SECRET).unwrap().secret;
        assert_eq!(encode_base32(&decoded), RFC_SECRET);
    }

    #[test]
    fn accepts_grouped_and_ambiguous_base32() {
        let grouped = parse_otp_input("GEZD GNBV GY3T QOJQ GEZD GNBV GY3T QOJQ").unwrap();
        let compact = parse_otp_input(RFC_SECRET).unwrap();
        assert_eq!(grouped.secret, compact.secret);
        assert_eq!(
            generate_code(&compact.secret, 0, "SHA1", 6).unwrap(),
            "755224"
        );
    }

    #[test]
    fn matches_rfc6238_totp_sha1_vector() {
        let config = parse_otp_input(&format!(
            "otpauth://totp/Test:user@example.com?secret={RFC_SECRET}&algorithm=SHA1&digits=8&period=30"
        ))
        .unwrap();
        assert_eq!(code_for(&config, 59).unwrap().code, "94287082");
    }

    #[test]
    fn parses_microsoft_oath_uri_and_sha256() {
        let config = parse_otp_input(&format!(
            "otpauth://totp/Microsoft:user%40example.com?secret={RFC_SECRET}&issuer=Microsoft&algorithm=SHA256&digits=6&period=60"
        ))
        .unwrap();
        assert_eq!(config.provider, "Microsoft");
        assert_eq!(config.algorithm, "SHA256");
        assert_eq!(config.period, 60);
        assert_eq!(config.account_name, "user@example.com");
    }

    #[test]
    fn supports_hotp_counter() {
        let config = parse_otp_input(&format!(
            "otpauth://hotp/Test:counter?secret={RFC_SECRET}&counter=1"
        ))
        .unwrap();
        assert_eq!(code_for(&config, 0).unwrap().code, "287082");
    }

    #[test]
    fn protects_secrets_with_windows_dpapi() {
        let secret = b"heaspot-dpapi-roundtrip-test";
        let encrypted = unsafe { dpapi_encrypt(secret) }.unwrap();
        assert_ne!(encrypted, secret);
        assert_eq!(unsafe { dpapi_decrypt(&encrypted) }.unwrap(), secret);
    }

    #[test]
    fn quick_add_accepts_plus_and_does_not_duplicate_secret() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE otp_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,name TEXT NOT NULL,note TEXT NOT NULL DEFAULT '',
                issuer TEXT NOT NULL DEFAULT '',secret_enc BLOB NOT NULL,otp_type TEXT NOT NULL DEFAULT 'totp',
                algorithm TEXT NOT NULL DEFAULT 'SHA1',digits INTEGER NOT NULL DEFAULT 6,period INTEGER NOT NULL DEFAULT 30,
                counter INTEGER NOT NULL DEFAULT 0,is_pinned INTEGER NOT NULL DEFAULT 0,is_archived INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
             );",
        )
        .unwrap();
        let first = quick_add_otp_account_with_conn(&conn, format!("+ {RFC_SECRET}")).unwrap();
        let second = quick_add_otp_account_with_conn(&conn, RFC_SECRET.into()).unwrap();
        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.account.id, second.account.id);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM otp_accounts", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn encrypted_backup_roundtrip_keeps_secret_note_dates_and_history() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE otp_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,name TEXT NOT NULL,note TEXT NOT NULL DEFAULT '',
                issuer TEXT NOT NULL DEFAULT '',secret_enc BLOB NOT NULL,otp_type TEXT NOT NULL,
                algorithm TEXT NOT NULL,digits INTEGER NOT NULL,period INTEGER NOT NULL,counter INTEGER NOT NULL,
                is_pinned INTEGER NOT NULL,is_archived INTEGER NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL
             );
             CREATE TABLE otp_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,account_id INTEGER NOT NULL,account_name TEXT NOT NULL,
                code_enc BLOB NOT NULL,generated_at INTEGER NOT NULL,valid_until INTEGER NOT NULL,copied INTEGER NOT NULL
             );",
        )
        .unwrap();
        let parsed = parse_otp_input(RFC_SECRET).unwrap();
        let encrypted_secret = unsafe { dpapi_encrypt(&parsed.secret) }.unwrap();
        conn.execute(
            "INSERT INTO otp_accounts(name,note,issuer,secret_enc,otp_type,algorithm,digits,period,counter,is_pinned,is_archived,created_at,updated_at)
             VALUES(?1,?2,?3,?4,'totp','SHA1',6,30,0,1,0,?5,?6)",
            params![
                "Mail thử nghiệm",
                "Chú thích phải đi theo backup",
                "Microsoft",
                encrypted_secret,
                "2026-07-20 10:00:00",
                "2026-07-20 11:00:00",
            ],
        )
        .unwrap();
        let encrypted_code = unsafe { dpapi_encrypt(b"123456") }.unwrap();
        conn.execute(
            "INSERT INTO otp_history(account_id,account_name,code_enc,generated_at,valid_until,copied)
             VALUES(1,'Mail thử nghiệm',?1,100,130,1)",
            params![encrypted_code],
        )
        .unwrap();

        let path = std::env::temp_dir().join(format!(
            "heaspot-otp-backup-test-{}.heaspot2fa",
            std::process::id()
        ));
        let path_text = path.to_string_lossy().into_owned();
        let exported =
            export_otp_backup_with_conn(&conn, path_text.clone(), "test-password-2fa".into())
                .unwrap();
        assert_eq!(exported.accounts, 1);
        assert_eq!(exported.history, 1);
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains(RFC_SECRET));
        assert!(!raw.contains("Chú thích phải đi theo backup"));

        conn.execute("DELETE FROM otp_history", []).unwrap();
        conn.execute("DELETE FROM otp_accounts", []).unwrap();
        let imported =
            import_otp_backup_with_conn(&mut conn, path_text.clone(), "test-password-2fa".into())
                .unwrap();
        assert_eq!(imported.accounts, 1);
        assert_eq!(imported.history, 1);
        let (name, note, created_at, updated_at, protected): (
            String,
            String,
            String,
            String,
            Vec<u8>,
        ) = conn
            .query_row(
                "SELECT name,note,created_at,updated_at,secret_enc FROM otp_accounts",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(name, "Mail thử nghiệm");
        assert_eq!(note, "Chú thích phải đi theo backup");
        assert_eq!(created_at, "2026-07-20 10:00:00");
        assert_eq!(updated_at, "2026-07-20 11:00:00");
        assert_eq!(unsafe { dpapi_decrypt(&protected) }.unwrap(), parsed.secret);

        let second =
            import_otp_backup_with_conn(&mut conn, path_text, "test-password-2fa".into()).unwrap();
        assert_eq!(second.accounts, 0);
        assert_eq!(second.skipped, 1);
        assert_eq!(second.history, 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn account_list_is_newest_first_and_secret_remains_retrievable() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE otp_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,name TEXT NOT NULL,note TEXT NOT NULL DEFAULT '',
                issuer TEXT NOT NULL DEFAULT '',secret_enc BLOB NOT NULL,otp_type TEXT NOT NULL DEFAULT 'totp',
                algorithm TEXT NOT NULL DEFAULT 'SHA1',digits INTEGER NOT NULL DEFAULT 6,
                period INTEGER NOT NULL DEFAULT 30,counter INTEGER NOT NULL DEFAULT 0,
                is_pinned INTEGER NOT NULL DEFAULT 0,is_archived INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,updated_at TEXT NOT NULL
            );",
        )
        .unwrap();
        let parsed = parse_otp_input(RFC_SECRET).unwrap();
        let protected = unsafe { dpapi_encrypt(&parsed.secret) }.unwrap();
        for (name, created_at) in [
            ("Cũ", "2026-07-25 09:00:00"),
            ("Mới cùng giây A", "2026-07-26 09:00:00"),
            ("Mới cùng giây B", "2026-07-26 09:00:00"),
        ] {
            conn.execute(
                "INSERT INTO otp_accounts(name,secret_enc,created_at,updated_at) VALUES(?1,?2,?3,?3)",
                params![name, protected, created_at],
            )
            .unwrap();
        }

        let accounts = list_otp_accounts_with_conn(&conn, false).unwrap();
        assert_eq!(
            accounts
                .iter()
                .map(|account| account.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Mới cùng giây B", "Mới cùng giây A", "Cũ"]
        );
        assert_eq!(account_secret(&conn, accounts[0].id).unwrap(), RFC_SECRET);
    }
}
