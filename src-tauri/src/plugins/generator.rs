//! Value Generator (`#`): hash MD5 / SHA1 / SHA256 cho chuỗi.
//! (UUID và Base64 xử lý ngay trên frontend cho tức thì.)

use md5::Digest;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[tauri::command]
pub fn hash_text(algo: String, text: String) -> Result<String, String> {
    match algo.as_str() {
        "md5" => Ok(hex(&md5::Md5::digest(text.as_bytes()))),
        "sha1" => Ok(hex(&sha1::Sha1::digest(text.as_bytes()))),
        "sha256" => Ok(hex(&sha2::Sha256::digest(text.as_bytes()))),
        _ => Err(format!("thuật toán không hỗ trợ: {algo}")),
    }
}
