$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class HeaCapture {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder b, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int s);
  [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
  [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
}
"@

[HeaCapture]::SetProcessDPIAware() | Out-Null
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root "src-tauri\target\release\heaspot.exe"
$out = Join-Path $root "docs\screenshots"
if (-not (Test-Path -LiteralPath $exe)) { throw "Build release first: $exe" }
New-Item -ItemType Directory -Force -Path $out | Out-Null

$profile = Join-Path $env:TEMP "heaspot-readme-profile"
$resolvedTemp = [IO.Path]::GetFullPath($env:TEMP)
$resolvedProfile = [IO.Path]::GetFullPath($profile)
if (-not $resolvedProfile.StartsWith($resolvedTemp, [StringComparison]::OrdinalIgnoreCase)) {
  throw "Unsafe temporary profile path"
}
Remove-Item -LiteralPath $profile -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $profile | Out-Null
$fulltextFixtureDir = Join-Path $env:USERPROFILE ".heaspot-smoke"
New-Item -ItemType Directory -Force -Path $fulltextFixtureDir | Out-Null
$fulltextFixture = Join-Path $fulltextFixtureDir "heaspot-fulltext-demo.txt"
[IO.File]::WriteAllText($fulltextFixture, "heaspot-demo-no-private-data - Hybrid full-text smoke-test fixture", (New-Object Text.UTF8Encoding($false)))

# Nền trung tính để vùng trong suốt của launcher không lộ ứng dụng/dữ liệu cá nhân.
$backdrop = New-Object Windows.Forms.Form
$backdrop.FormBorderStyle = "None"
$backdrop.StartPosition = "Manual"
$backdrop.Bounds = [Windows.Forms.Screen]::PrimaryScreen.Bounds
$backdrop.BackColor = [Drawing.Color]::FromArgb(14, 16, 20)
$backdrop.ShowInTaskbar = $false
$backdrop.TopMost = $true
$backdrop.Show()
$backdrop.BringToFront()
$backdrop.Activate()
[Windows.Forms.Application]::DoEvents()
$backdrop.TopMost = $false
[Windows.Forms.Application]::DoEvents()

[Windows.Forms.Clipboard]::SetText("HeaSpot demo clipboard - safe documentation content")
$psi = New-Object Diagnostics.ProcessStartInfo
$psi.FileName = $exe
$psi.UseShellExecute = $false
$psi.Environment["HEASPOT_DATA_DIR"] = $profile
$app = [Diagnostics.Process]::Start($psi)
Start-Sleep -Seconds 12
$script:HeaPids = @(Get-Process heaspot -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)

$KeyUp = 0x2
function Key([byte]$key) {
  [HeaCapture]::keybd_event($key, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 35
  [HeaCapture]::keybd_event($key, 0, $KeyUp, [UIntPtr]::Zero)
}
function ReleaseModifiers {
  foreach ($key in @(0x5B, 0x5C, 0x12, 0x11, 0x10)) {
    [HeaCapture]::keybd_event([byte]$key, 0, $KeyUp, [UIntPtr]::Zero)
  }
}
function AltSpace {
  [HeaCapture]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
  [HeaCapture]::keybd_event(0x20, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 45
  [HeaCapture]::keybd_event(0x20, 0, $KeyUp, [UIntPtr]::Zero)
  [HeaCapture]::keybd_event(0x12, 0, $KeyUp, [UIntPtr]::Zero)
}
function WinV {
  [HeaCapture]::keybd_event(0x5B, 0, 0, [UIntPtr]::Zero)
  [HeaCapture]::keybd_event(0x56, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 45
  [HeaCapture]::keybd_event(0x56, 0, $KeyUp, [UIntPtr]::Zero)
  [HeaCapture]::keybd_event(0x5B, 0, $KeyUp, [UIntPtr]::Zero)
}
function AltF4 {
  [HeaCapture]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
  [HeaCapture]::keybd_event(0x73, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 45
  [HeaCapture]::keybd_event(0x73, 0, $KeyUp, [UIntPtr]::Zero)
  [HeaCapture]::keybd_event(0x12, 0, $KeyUp, [UIntPtr]::Zero)
}
function FrontPid {
  $pidValue = 0
  [HeaCapture]::GetWindowThreadProcessId([HeaCapture]::GetForegroundWindow(), [ref]$pidValue) | Out-Null
  return $pidValue
}
function FrontTitle {
  $builder = New-Object Text.StringBuilder 256
  [HeaCapture]::GetWindowText([HeaCapture]::GetForegroundWindow(), $builder, $builder.Capacity) | Out-Null
  return $builder.ToString()
}
function IsHeaFront { return $script:HeaPids -contains (FrontPid) }
function IsLauncherFront { return (IsHeaFront) -and ((FrontTitle) -eq "HeaSpot") }
function FocusHeaSpot {
  foreach ($pidValue in $script:HeaPids) {
    $process = Get-Process -Id $pidValue -ErrorAction SilentlyContinue
    if ($process -and $process.MainWindowHandle -ne [IntPtr]::Zero) {
      [HeaCapture]::SetForegroundWindow($process.MainWindowHandle) | Out-Null
      Start-Sleep -Milliseconds 350
      if (IsHeaFront) { return $true }
    }
  }
  return $false
}
function OpenLauncher {
  if (IsLauncherFront) { return $true }
  for ($i = 0; $i -lt 5; $i++) {
    ReleaseModifiers
    Key 0x1B
    AltSpace
    Start-Sleep -Milliseconds 500
    if (IsLauncherFront) { return $true }
  }
  return $false
}
function TypeQuery([string]$query) {
  Key 0x1B
  Start-Sleep -Milliseconds 250
  if (-not (OpenLauncher)) { throw "Cannot focus HeaSpot for query: $query" }
  [Windows.Forms.SendKeys]::SendWait("^a")
  Start-Sleep -Milliseconds 60
  [Windows.Forms.SendKeys]::SendWait("{DELETE}")
  if ($query) {
    # Paste thay vì gõ từng phím để không bị bộ gõ Telex biến đổi query/email.
    [Windows.Forms.Clipboard]::SetText($query)
    [Windows.Forms.SendKeys]::SendWait("^v")
  }
  # Để con trỏ ngoài launcher, tránh hover vô tình đổi selectedIndex khi list resize.
  [HeaCapture]::SetCursorPos(2, 2) | Out-Null
}
function WindowRect {
  $handle = [HeaCapture]::GetForegroundWindow()
  $rect = New-Object HeaCapture+RECT
  # GetWindowRect đáng tin cậy hơn DWM bounds với WebView trong suốt của Tauri.
  [HeaCapture]::GetWindowRect($handle, [ref]$rect) | Out-Null
  return $rect
}
function WaitForHeight([int]$minimum, [int]$timeoutSeconds) {
  $deadline = (Get-Date).AddSeconds($timeoutSeconds)
  do {
    if (IsHeaFront) {
      $rect = WindowRect
      if (($rect.Bottom - $rect.Top) -ge $minimum) { return $true }
    }
    Start-Sleep -Milliseconds 250
  } while ((Get-Date) -lt $deadline)
  return $false
}
function Shoot([string]$name) {
  Start-Sleep -Milliseconds 350
  if (-not (IsHeaFront)) {
    $frontPid = FrontPid
    $frontProcess = Get-Process -Id $frontPid -ErrorAction SilentlyContinue
    $alive = @(Get-Process heaspot -ErrorAction SilentlyContinue).Count
    throw "HeaSpot lost foreground while capturing ${name}; front=$($frontProcess.ProcessName) pid=$frontPid title='$(FrontTitle)' heaspotAlive=$alive"
  }
  # Đặt nền trung tính ngay dưới app bằng Z-order, không activate/cướp focus.
  $noMoveSizeActivate = 0x0013
  [HeaCapture]::SetWindowPos($backdrop.Handle, [IntPtr](-1), 0, 0, 0, 0, $noMoveSizeActivate) | Out-Null
  [HeaCapture]::SetWindowPos([HeaCapture]::GetForegroundWindow(), [IntPtr](-1), 0, 0, 0, 0, $noMoveSizeActivate) | Out-Null
  $rect = WindowRect
  $width = $rect.Right - $rect.Left
  $height = $rect.Bottom - $rect.Top
  if ($width -lt 500 -or $height -lt 60 -or $width -gt 1800 -or $height -gt 1200) {
    throw "Invalid capture bounds for ${name}: ${width}x${height}"
  }
  $bitmap = New-Object Drawing.Bitmap $width, $height
  $graphics = [Drawing.Graphics]::FromImage($bitmap)
  $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object Drawing.Size $width, $height))
  $path = Join-Path $out "$name.png"
  $bitmap.Save($path, [Drawing.Imaging.ImageFormat]::Png)
  $graphics.Dispose()
  $bitmap.Dispose()
  Write-Host "Captured $name (${width}x${height})"
}

try {
  TypeQuery ""
  Start-Sleep -Seconds 1
  Shoot "search-launcher"

  TypeQuery "notepad"
  if (-not (WaitForHeight 150 30)) { throw "App Search did not produce results in 30 seconds" }
  Shoot "app-search"
  Key 0x27
  if (-not (WaitForHeight 300 5)) { throw "Context Menu did not expand" }
  Shoot "context-menu"
  Key 0x25

  TypeQuery "conv 60 km/h to m/s"
  Start-Sleep -Seconds 1
  Shoot "converter"

  TypeQuery "= sin(pi/2)"
  Start-Sleep -Seconds 1
  Shoot "calculator"

  TypeQuery "time tokyo"
  Start-Sleep -Seconds 1
  Shoot "timezones"

  TypeQuery "tr hello"
  Start-Sleep -Seconds 6
  Shoot "translate"

  TypeQuery "wiki artificial intelligence"
  Start-Sleep -Seconds 6
  Shoot "wikipedia"

  TypeQuery "formula kinetic energy"
  Start-Sleep -Seconds 4
  Shoot "formula"

  TypeQuery "ps edge"
  Start-Sleep -Seconds 3
  Shoot "task-manager"

  TypeQuery "in heaspot-demo-no-private-data"
  Start-Sleep -Seconds 12
  Shoot "fulltext"

  TypeQuery "g photosynthesis"
  Start-Sleep -Seconds 5
  Shoot "google-quick"

  TypeQuery "snip mail student@heaspot.local"
  Key 0x0D
  Start-Sleep -Seconds 1
  Shoot "snippets"

  Key 0x1B
  Start-Sleep -Milliseconds 300
  [Windows.Forms.Clipboard]::SetText("HeaSpot demo clipboard - safe documentation content")
  Start-Sleep -Seconds 1
  WinV
  Start-Sleep -Seconds 2
  Shoot "clipboard"

  # Đưa một ảnh demo vào clipboard để kiểm tra preview và zoom ảnh.
  $demoImage = [Drawing.Image]::FromFile((Join-Path $out "converter.png"))
  $copy = New-Object Drawing.Bitmap $demoImage
  $demoImage.Dispose()
  [Windows.Forms.Clipboard]::SetImage($copy)
  $copy.Dispose()
  Start-Sleep -Seconds 2
  $rect = WindowRect
  [HeaCapture]::SetCursorPos($rect.Left + [int](($rect.Right-$rect.Left)*0.72), $rect.Top + [int](($rect.Bottom-$rect.Top)*0.52)) | Out-Null
  [HeaCapture]::mouse_event(0x0800, 0, 0, 360, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 500
  Shoot "image-zoom"

  Key 0x1B
  TypeQuery "settings"
  Key 0x0D
  Start-Sleep -Seconds 2
  Shoot "settings"
  $rect = WindowRect
  [HeaCapture]::SetCursorPos($rect.Left + [int](($rect.Right-$rect.Left)*0.72), $rect.Top + [int](($rect.Bottom-$rect.Top)*0.72)) | Out-Null
  for ($i = 0; $i -lt 5; $i++) {
    [HeaCapture]::mouse_event(0x0800, 0, 0, [uint32]4294966936, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 140
  }
  Shoot "clear-cache"
  # Tab Keyword & Hotkey tính năng.
  [HeaCapture]::SetCursorPos($rect.Left + 105, $rect.Top + 150) | Out-Null
  [HeaCapture]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
  [HeaCapture]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 700
  Shoot "feature-hotkeys"

  # OCR: cấp ảnh chữ demo trực tiếp sau khi lệnh mở Screen Snipping.
  AltF4
  Start-Sleep -Milliseconds 600
  TypeQuery "ocr"
  Key 0x0D
  Start-Sleep -Seconds 1
  $ocrBitmap = New-Object Drawing.Bitmap 1000, 260
  $g = [Drawing.Graphics]::FromImage($ocrBitmap)
  $g.Clear([Drawing.Color]::White)
  $font = New-Object Drawing.Font "Segoe UI", 30
  $brush = [Drawing.Brushes]::Black
  $ocrText = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String("SGVhU3BvdCBuaOG6rW4gZOG6oW5nIHRp4bq/bmcgVmnhu4d0IGNow61uaCB4w6FjLgpI4buNYyB04bqtcCB0aMO0bmcgbWluaCB2w6AgbMOgbSB2aeG7h2MgbmhhbmggaMahbi4="))
  $g.DrawString($ocrText, $font, $brush, 35, 35)
  [Windows.Forms.Clipboard]::SetImage($ocrBitmap)
  $g.Dispose(); $font.Dispose(); $ocrBitmap.Dispose()
  Key 0x1B
  Start-Sleep -Seconds 10
  if (-not (IsHeaFront)) { [void](FocusHeaSpot) }
  Shoot "ocr"
} finally {
  Get-Process heaspot -ErrorAction SilentlyContinue | Where-Object { $script:HeaPids -contains $_.Id } |
    Stop-Process -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $fulltextFixture -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $fulltextFixtureDir -Force -ErrorAction SilentlyContinue
  Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -eq "Everything.exe" -and $_.CommandLine -like "*$profile*" } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
  $backdrop.Close(); $backdrop.Dispose()
  Start-Sleep -Milliseconds 300
  Remove-Item -LiteralPath $profile -Recurse -Force -ErrorAction SilentlyContinue
}
