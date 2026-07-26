//! Registry Plugin (`:`): duyệt key trong Windows Registry và
//! mở thẳng Registry Editor tại đúng vị trí.

use serde::Serialize;
use winreg::enums::*;
use winreg::RegKey;

#[derive(Serialize)]
pub struct RegKeyInfo {
    pub path: String,
    pub name: String,
}

fn parse_root(root: &str) -> Option<(RegKey, &'static str)> {
    match root.to_uppercase().as_str() {
        "HKCU" | "HKEY_CURRENT_USER" => {
            Some((RegKey::predef(HKEY_CURRENT_USER), "HKEY_CURRENT_USER"))
        }
        "HKLM" | "HKEY_LOCAL_MACHINE" => {
            Some((RegKey::predef(HKEY_LOCAL_MACHINE), "HKEY_LOCAL_MACHINE"))
        }
        "HKCR" | "HKEY_CLASSES_ROOT" => {
            Some((RegKey::predef(HKEY_CLASSES_ROOT), "HKEY_CLASSES_ROOT"))
        }
        "HKU" | "HKEY_USERS" => Some((RegKey::predef(HKEY_USERS), "HKEY_USERS")),
        "HKCC" | "HKEY_CURRENT_CONFIG" => {
            Some((RegKey::predef(HKEY_CURRENT_CONFIG), "HKEY_CURRENT_CONFIG"))
        }
        _ => None,
    }
}

/// Gõ `: hkcu\software\mi` -> liệt kê các subkey của HKCU\Software bắt đầu bằng "mi"
#[tauri::command]
pub fn registry_search(query: String) -> Vec<RegKeyInfo> {
    let q = query.trim().trim_start_matches('\\');
    if q.is_empty() {
        // Gợi ý các root
        return ["HKCU", "HKLM", "HKCR", "HKU", "HKCC"]
            .iter()
            .map(|r| RegKeyInfo {
                path: parse_root(r)
                    .map(|(_, full)| full.to_string())
                    .unwrap_or_default(),
                name: r.to_string(),
            })
            .collect();
    }

    let mut parts: Vec<&str> = q.split('\\').collect();
    let root_str = parts.remove(0);
    let Some((root, root_full)) = parse_root(root_str) else {
        return Vec::new();
    };

    // Phần cuối là prefix đang gõ dở (trừ khi query kết thúc bằng '\')
    let (parent_parts, prefix) = if q.ends_with('\\') {
        (parts.as_slice(), String::new())
    } else if parts.is_empty() {
        (&[] as &[&str], String::new())
    } else {
        let p = parts[parts.len() - 1].to_lowercase();
        (&parts[..parts.len() - 1], p)
    };

    let parent_path = parent_parts.join("\\");
    let parent = if parent_path.is_empty() {
        root
    } else {
        match root.open_subkey(&parent_path) {
            Ok(k) => k,
            Err(_) => return Vec::new(),
        }
    };

    let full_parent = if parent_path.is_empty() {
        root_full.to_string()
    } else {
        format!("{root_full}\\{parent_path}")
    };

    parent
        .enum_keys()
        .flatten()
        .filter(|k| prefix.is_empty() || k.to_lowercase().starts_with(&prefix))
        .take(15)
        .map(|k| RegKeyInfo {
            path: format!("{full_parent}\\{k}"),
            name: k,
        })
        .collect()
}

/// Mở Registry Editor tại đúng key (set LastKey rồi khởi động regedit)
#[tauri::command]
pub fn open_regedit(path: String) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Applets\Regedit")
        .map_err(|e| e.to_string())?;
    key.set_value("LastKey", &format!("Computer\\{path}"))
        .map_err(|e| e.to_string())?;
    std::process::Command::new("regedit")
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
