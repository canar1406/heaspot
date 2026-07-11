use md5::Digest;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::Manager;

fn tesseract_executable(app: &tauri::AppHandle) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(resources) = app.path().resource_dir() {
        candidates.push(resources.join("ocr").join("tesseract.exe"));
    }
    if let Ok(path) = std::env::var("HEASPOT_TESSERACT_PATH") {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(root) = std::env::var("ProgramFiles") {
        candidates.push(PathBuf::from(root).join("Tesseract-OCR").join("tesseract.exe"));
    }
    if let Ok(root) = std::env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(root).join("Programs").join("Tesseract-OCR").join("tesseract.exe"));
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn recognize_vietnamese_with_tesseract(app: &tauri::AppHandle, path: &Path) -> Result<Option<String>, String> {
    let Some(exe) = tesseract_executable(app) else { return Ok(None) };
    let mut command = std::process::Command::new(exe);
    command.args([path.as_os_str(), "stdout".as_ref(), "-l".as_ref(), "vie+eng".as_ref(), "--oem".as_ref(), "1".as_ref(), "--psm".as_ref(), "6".as_ref()]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output().map_err(|e| format!("không chạy được Tesseract OCR: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Tesseract OCR tiếng Việt chưa sẵn sàng: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!text.is_empty()).then_some(text))
}

/// Mở Windows screen snipping, chờ ảnh mới trong clipboard rồi OCR offline bằng Windows.Media.Ocr.
#[tauri::command]
pub async fn capture_ocr(app: tauri::AppHandle) -> Result<String, String> {
    if let Some(win) = app.get_webview_window("main") { let _ = win.hide(); }
    tauri::async_runtime::spawn_blocking(move || {
        let before_seq = unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() };
        std::process::Command::new("explorer.exe").arg("ms-screenclip:").spawn()
            .map_err(|e| format!("không mở được công cụ chụp vùng: {e}"))?;
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        let mut captured = None;
        for _ in 0..200 {
            std::thread::sleep(Duration::from_millis(300));
            let seq = unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() };
            if seq == before_seq { continue; }
            if let Ok(img) = clipboard.get_image() {
                let hash = format!("{:x}", md5::Md5::digest(&img.bytes));
                let path = std::env::temp_dir().join(format!("heaspot_ocr_{hash}.png"));
                let rgba = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.to_vec())
                    .ok_or("dữ liệu ảnh clipboard không hợp lệ")?;
                // Windows OCR nhận chữ nhỏ/dấu tiếng Việt tốt hơn đáng kể khi ảnh được
                // phóng to, tăng tương phản nhẹ và làm nét trước khi đưa vào engine.
                let longest = rgba.width().max(rgba.height());
                let scale = if longest < 1_600 { 2 } else { 1 };
                let prepared = if scale > 1 {
                    image::imageops::resize(
                        &rgba,
                        rgba.width() * scale,
                        rgba.height() * scale,
                        image::imageops::FilterType::Lanczos3,
                    )
                } else {
                    rgba
                };
                let prepared = image::imageops::contrast(&prepared, 18.0);
                let prepared = image::imageops::unsharpen(&prepared, 1.0, 1);
                prepared.save(&path).map_err(|e| e.to_string())?;
                captured = Some(path);
                break;
            }
        }
        let path = captured.ok_or("đã hủy chụp hoặc không nhận được ảnh sau 60 giây")?;
        if let Some(text) = recognize_vietnamese_with_tesseract(&app, &path)? {
            let _ = std::fs::remove_file(path);
            return Ok(text);
        }
        let script = r#"
try {
  Add-Type -AssemblyName System.Runtime.WindowsRuntime
  $null = [Windows.Storage.StorageFile, Windows.Storage, ContentType=WindowsRuntime]
  $null = [Windows.Storage.Streams.IRandomAccessStream, Windows.Storage.Streams, ContentType=WindowsRuntime]
  $null = [Windows.Graphics.Imaging.BitmapDecoder, Windows.Graphics.Imaging, ContentType=WindowsRuntime]
  $null = [Windows.Graphics.Imaging.SoftwareBitmap, Windows.Graphics.Imaging, ContentType=WindowsRuntime]
  $null = [Windows.Media.Ocr.OcrEngine, Windows.Media.Ocr, ContentType=WindowsRuntime]
  $null = [Windows.Media.Ocr.OcrResult, Windows.Media.Ocr, ContentType=WindowsRuntime]
  $null = [Windows.Globalization.Language, Windows.Globalization, ContentType=WindowsRuntime]
  function Await($op, $type) {
    $m = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
      $_.Name -eq 'AsTask' -and $_.IsGenericMethod -and $_.GetParameters().Count -eq 1
    })[0]
    $task = $m.MakeGenericMethod($type).Invoke($null, @($op))
    $task.Wait()
    $task.Result
  }
  $file = Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync($env:WINSPOT_OCR_PATH)) ([Windows.Storage.StorageFile])
  $stream = Await ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
  $decoder = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
  $bitmap = Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
  $vi = [Windows.Media.Ocr.OcrEngine]::AvailableRecognizerLanguages | Where-Object { $_.LanguageTag -like 'vi*' } | Select-Object -First 1
  if ($null -eq $vi) {
    throw 'Máy chưa có engine OCR tiếng Việt. Cài Tesseract 5 và model vie, rồi mở lại HeaSpot.'
  }
  $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage($vi)
  if ($null -eq $engine) { throw 'Không khởi tạo được model OCR tiếng Việt của Windows' }
  $result = Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])
  [string]$result.Text
} catch { Write-Error $_; exit 1 }
"#;
        let path_str = path.to_string_lossy().to_string();
        let result = crate::commands::run_hidden_ps(script, &[("WINSPOT_OCR_PATH", &path_str)]);
        let _ = std::fs::remove_file(path);
        let text = result?.trim().to_string();
        if text.is_empty() { Err("không nhận dạng được chữ trong vùng đã chụp".into()) } else { Ok(text) }
    }).await.map_err(|e| e.to_string())?
}
