param(
    [Parameter(Mandatory = $true)]
    [string]$Exe
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms

Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;

namespace HeaSpotFocusSmoke {
    public static class Native {
        public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);

        [DllImport("user32.dll")]
        public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);
        [DllImport("user32.dll")]
        public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
        [DllImport("user32.dll")]
        public static extern bool IsWindowVisible(IntPtr hwnd);
        [DllImport("user32.dll")]
        public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")]
        public static extern IntPtr GetAncestor(IntPtr hwnd, uint flags);
        [DllImport("user32.dll")]
        public static extern bool SetForegroundWindow(IntPtr hwnd);
        [DllImport("user32.dll")]
        public static extern bool BringWindowToTop(IntPtr hwnd);
        [DllImport("user32.dll")]
        public static extern IntPtr SetFocus(IntPtr hwnd);
        [DllImport("user32.dll")]
        public static extern bool ShowWindow(IntPtr hwnd, int command);
        [DllImport("user32.dll")]
        public static extern bool AttachThreadInput(uint first, uint second, bool attach);
        [DllImport("kernel32.dll")]
        public static extern uint GetCurrentThreadId();
        [DllImport("user32.dll")]
        public static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extraInfo);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint SendInput(uint count, INPUT[] inputs, int size);

        [StructLayout(LayoutKind.Sequential)]
        public struct INPUT {
            public uint type;
            public INPUTUNION data;
        }
        [StructLayout(LayoutKind.Explicit, Size = 32)]
        public struct INPUTUNION {
            [FieldOffset(0)] public KEYBDINPUT keyboard;
        }
        [StructLayout(LayoutKind.Sequential)]
        public struct KEYBDINPUT {
            public ushort virtualKey;
            public ushort scanCode;
            public uint flags;
            public uint time;
            public UIntPtr extraInfo;
        }

        public static bool SendVirtualKey(ushort virtualKey) {
            var inputs = new INPUT[2];
            inputs[0].type = 1;
            inputs[0].data.keyboard.virtualKey = virtualKey;
            inputs[1].type = 1;
            inputs[1].data.keyboard.virtualKey = virtualKey;
            inputs[1].data.keyboard.flags = 2;
            return SendInput(2, inputs, Marshal.SizeOf(typeof(INPUT))) == 2;
        }

        public static void ForceForegroundWindow(IntPtr target) {
            var foreground = GetForegroundWindow();
            uint ignored;
            var currentThread = GetCurrentThreadId();
            var targetThread = GetWindowThreadProcessId(target, out ignored);
            var foregroundThread = foreground == IntPtr.Zero
                ? 0
                : GetWindowThreadProcessId(foreground, out ignored);
            var attachedTarget = targetThread != 0 && targetThread != currentThread &&
                AttachThreadInput(currentThread, targetThread, true);
            var attachedForeground = foregroundThread != 0 && foregroundThread != currentThread &&
                foregroundThread != targetThread && AttachThreadInput(currentThread, foregroundThread, true);
            ShowWindow(target, 5);
            BringWindowToTop(target);
            SetForegroundWindow(target);
            SetFocus(target);
            if (attachedForeground) AttachThreadInput(currentThread, foregroundThread, false);
            if (attachedTarget) AttachThreadInput(currentThread, targetThread, false);
        }

        public static IntPtr FindExactTitle(uint expectedProcessId, string expectedTitle) {
            IntPtr found = IntPtr.Zero;
            EnumWindows((hwnd, _) => {
                uint processId;
                GetWindowThreadProcessId(hwnd, out processId);
                if (processId != expectedProcessId) return true;
                var title = new StringBuilder(512);
                GetWindowText(hwnd, title, title.Capacity);
                if (title.ToString() == expectedTitle) {
                    found = hwnd;
                    return false;
                }
                return true;
            }, IntPtr.Zero);
            return found;
        }
    }
}
'@

function Send-Key([byte]$key) {
    [HeaSpotFocusSmoke.Native]::keybd_event($key, 0, 0, [UIntPtr]::Zero)
    [HeaSpotFocusSmoke.Native]::keybd_event($key, 0, 2, [UIntPtr]::Zero)
}

function Send-AltSpace {
    [HeaSpotFocusSmoke.Native]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
    Send-Key 0x20
    [HeaSpotFocusSmoke.Native]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
}

function Send-WinV {
    [HeaSpotFocusSmoke.Native]::keybd_event(0x5B, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 45
    [HeaSpotFocusSmoke.Native]::keybd_event(0x56, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 35
    [HeaSpotFocusSmoke.Native]::keybd_event(0x56, 0, 2, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 35
    [HeaSpotFocusSmoke.Native]::keybd_event(0x5B, 0, 2, [UIntPtr]::Zero)
}

function Get-ForegroundProcessId {
    $foregroundProcessId = [uint32]0
    $foreground = [HeaSpotFocusSmoke.Native]::GetForegroundWindow()
    [HeaSpotFocusSmoke.Native]::GetWindowThreadProcessId($foreground, [ref]$foregroundProcessId) | Out-Null
    return $foregroundProcessId
}

function Wait-Until([scriptblock]$Condition, [int]$TimeoutMs) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while ($timer.ElapsedMilliseconds -lt $TimeoutMs) {
        [Windows.Forms.Application]::DoEvents()
        if (& $Condition) { return $timer.ElapsedMilliseconds }
        Start-Sleep -Milliseconds 15
    }
    throw "Timeout after ${TimeoutMs}ms"
}

function Focus-TestWindow {
    $focusForm.BringToFront()
    $focusForm.Activate() | Out-Null
    $focusForm.Focus() | Out-Null
    [Windows.Forms.Application]::DoEvents()
    [HeaSpotFocusSmoke.Native]::ForceForegroundWindow($target)
    try {
        Wait-Until { [HeaSpotFocusSmoke.Native]::GetForegroundWindow() -eq $target } 1200 | Out-Null
    }
    catch {
        throw "Focus target did not become foreground (actual $([HeaSpotFocusSmoke.Native]::GetForegroundWindow()))"
    }
}

$dataDir = Join-Path $env:TEMP "heaspot-focus-smoke-$PID"
$app = $null
$focusForm = $null
try {
    New-Item -ItemType Directory -Path $dataDir -ErrorAction Stop | Out-Null
    $env:HEASPOT_DATA_DIR = $dataDir
    $app = Start-Process -FilePath $Exe -WorkingDirectory (Split-Path $Exe) -WindowStyle Hidden -PassThru
    Remove-Item Env:HEASPOT_DATA_DIR

    $main = [IntPtr]::Zero
    Wait-Until {
        $script:main = [HeaSpotFocusSmoke.Native]::FindExactTitle([uint32]$app.Id, 'HeaSpot')
        $script:main -ne [IntPtr]::Zero
    } 10000 | Out-Null

    $focusForm = New-Object Windows.Forms.Form
    $focusForm.Text = 'HeaSpot Focus Smoke Target'
    $focusForm.Width = 360
    $focusForm.Height = 140
    $focusForm.StartPosition = 'CenterScreen'
    $focusForm.Show()
    [Windows.Forms.Application]::DoEvents()
    $target = $focusForm.Handle
    Start-Sleep -Milliseconds 1500

    $focusLatencies = @()
    for ($iteration = 1; $iteration -le 8; $iteration++) {
        Focus-TestWindow
        Start-Sleep -Milliseconds 80
        Send-WinV
        Wait-Until {
            [HeaSpotFocusSmoke.Native]::IsWindowVisible($main) -and
            (Get-ForegroundProcessId) -eq [uint32]$app.Id
        } 2500 | Out-Null

        # Switch away inside the old 700 ms grace/race window.
        Start-Sleep -Milliseconds (20 + (($iteration * 23) % 170))
        Focus-TestWindow
        try {
            $latency = Wait-Until { -not [HeaSpotFocusSmoke.Native]::IsWindowVisible($main) } 1200
        }
        catch {
            throw "Launcher remained visible after confirmed focus loss on iteration $iteration"
        }
        $focusLatencies += $latency

        # The stale 350 ms fallback must not resurrect the launcher.
        $watch = [Diagnostics.Stopwatch]::StartNew()
        while ($watch.ElapsedMilliseconds -lt 750) {
            if ([HeaSpotFocusSmoke.Native]::IsWindowVisible($main)) {
                throw "Launcher reappeared after focus loss on iteration $iteration"
            }
            Start-Sleep -Milliseconds 20
        }
    }

    Focus-TestWindow
    Send-WinV
    Wait-Until {
        [HeaSpotFocusSmoke.Native]::IsWindowVisible($main) -and
        (Get-ForegroundProcessId) -eq [uint32]$app.Id
    } 2500 | Out-Null
    Start-Sleep -Milliseconds 35
    $escapeForeground = [HeaSpotFocusSmoke.Native]::GetForegroundWindow()
    $escapeRoot = [HeaSpotFocusSmoke.Native]::GetAncestor($escapeForeground, 2)
    "ESCAPE_MAIN_HWND=$main"
    "ESCAPE_FOREGROUND_HWND=$escapeForeground"
    "ESCAPE_ROOT_HWND=$escapeRoot"
    "ESCAPE_FOREGROUND_PID=$(Get-ForegroundProcessId)"
    if (-not [HeaSpotFocusSmoke.Native]::SendVirtualKey(0x1B)) {
        throw 'SendInput could not inject Escape'
    }
    [Windows.Forms.Application]::DoEvents()
    $escapeLatency = Wait-Until { -not [HeaSpotFocusSmoke.Native]::IsWindowVisible($main) } 800
    Start-Sleep -Milliseconds 500
    if ([HeaSpotFocusSmoke.Native]::IsWindowVisible($main)) {
        throw 'Launcher reappeared after native Escape'
    }

    [pscustomobject]@{
        FocusIterations = 8
        FocusHideMaxMs = ($focusLatencies | Measure-Object -Maximum).Maximum
        FocusHideAverageMs = [Math]::Round(($focusLatencies | Measure-Object -Average).Average, 1)
        EscapeHideMs = $escapeLatency
        Reappeared = $false
    } | Format-List
}
finally {
    Remove-Item Env:HEASPOT_DATA_DIR -ErrorAction SilentlyContinue
    if ($app -and -not $app.HasExited) { Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue }
    if ($focusForm) {
        $focusForm.Close()
        $focusForm.Dispose()
    }
    if (Test-Path -LiteralPath $dataDir) {
        $resolved = (Resolve-Path -LiteralPath $dataDir).Path
        $tempRoot = (Resolve-Path -LiteralPath $env:TEMP).Path
        if ($resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -and
            (Split-Path -Leaf $resolved).StartsWith('heaspot-focus-smoke-')) {
            Remove-Item -LiteralPath $resolved -Recurse -Force
        }
    }
}
