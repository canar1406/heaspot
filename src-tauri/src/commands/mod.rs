pub mod capacities;
pub mod clipboard;
pub mod currency;
pub mod knowledge;
pub mod ocr;
pub mod search;
pub mod settings;
pub mod study;
pub mod snippets;
pub mod system;

/// Chạy một đoạn PowerShell ẩn (không hiện cửa sổ), trả về stdout.
/// `envs` truyền dữ liệu vào script qua biến môi trường để né quoting hell.
pub(crate) fn run_hidden_ps(
    script: &str,
    envs: &[(&str, &str)],
) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // Windows PowerShell 5.1 mặc định ghi redirected stdout theo code page hệ thống.
    // Ép UTF-8 không BOM để Rust luôn giải mã đúng tiếng Việt từ JSON/API.
    let utf8_script = format!(
        "$utf8 = New-Object System.Text.UTF8Encoding($false); \
         [Console]::OutputEncoding = $utf8; $OutputEncoding = $utf8; {script}"
    );
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &utf8_script])
        .creation_flags(CREATE_NO_WINDOW);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let output = cmd.output().map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn powershell_stdout_is_utf8() {
        let output = super::run_hidden_ps("'Tiếng Việt: xin chào'", &[]).unwrap();
        assert!(output.contains("Tiếng Việt: xin chào"), "output={output:?}");
    }
}
