use md5::Digest;
use std::time::Duration;
use tauri::Manager;

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
                let path = std::env::temp_dir().join(format!("winspot_ocr_{hash}.png"));
                let rgba = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.to_vec())
                    .ok_or("dữ liệu ảnh clipboard không hợp lệ")?;
                rgba.save(&path).map_err(|e| e.to_string())?;
                captured = Some(path);
                break;
            }
        }
        let path = captured.ok_or("đã hủy chụp hoặc không nhận được ảnh sau 60 giây")?;
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
  $engine = if ($null -ne $vi) { [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage($vi) } else { [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages() }
  if ($null -eq $engine) { throw 'Windows OCR không hỗ trợ ngôn ngữ hiện tại' }
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
