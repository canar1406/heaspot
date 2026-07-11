pub mod capacities;
pub mod clipboard;
pub mod search;
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
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let output = cmd.output().map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
