$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
$target = Join-Path $projectRoot "src-tauri\resources\heaspot-ocr"
$tesseract = Join-Path $target "tesseract.exe"
$vie = Join-Path $target "tessdata\vie.traineddata"
$eng = Join-Path $target "tessdata\eng.traineddata"

$modelsReady = (Test-Path $vie) -and (Test-Path $eng) -and
    ((Get-FileHash -LiteralPath $vie -Algorithm SHA256).Hash -eq "B6B49293D95D0B6DBD8780174627E82C75BE957B6F4ED9862155540D6B00BB45") -and
    ((Get-FileHash -LiteralPath $eng -Algorithm SHA256).Hash -eq "8280AED0782FE27257A68EA10FE7EF324CA0F8D85BD2FD145D1C2B560BCB66BA")
if ((Test-Path $tesseract) -and $modelsReady) {
    Write-Host "Bundled OCR runtime is ready."
    exit 0
}

$installerUrl = "https://github.com/tesseract-ocr/tesseract/releases/download/5.5.0/tesseract-ocr-w64-setup-5.5.0.20241111.exe"
$installerSha256 = "F3FC4236425B690C8BE756F35793F77394EE004BE0A6460A440C754D892F68BC"
$models = @(
    @("eng", "https://github.com/tesseract-ocr/tessdata_best/raw/main/eng.traineddata", "8280AED0782FE27257A68EA10FE7EF324CA0F8D85BD2FD145D1C2B560BCB66BA"),
    @("vie", "https://github.com/tesseract-ocr/tessdata_best/raw/main/vie.traineddata", "B6B49293D95D0B6DBD8780174627E82C75BE957B6F4ED9862155540D6B00BB45")
)

$temp = Join-Path ([System.IO.Path]::GetTempPath()) "heaspot-tesseract-5.5.0.exe"
$installRoot = Join-Path ([System.IO.Path]::GetTempPath()) "heaspot-tesseract-runtime"
$modelRoot = Join-Path ([System.IO.Path]::GetTempPath()) "heaspot-ocr-models"
try {
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    Remove-Item -LiteralPath $installRoot -Recurse -Force -ErrorAction SilentlyContinue
    $runtimeSource = Join-Path $env:ProgramFiles "Tesseract-OCR"
    if (-not (Test-Path (Join-Path $runtimeSource "tesseract.exe"))) {
        New-Item -ItemType Directory -Force -Path $installRoot | Out-Null
        Invoke-WebRequest -Uri $installerUrl -OutFile $temp -UseBasicParsing
        if ((Get-FileHash -LiteralPath $temp -Algorithm SHA256).Hash -ne $installerSha256) {
            throw "Tesseract installer checksum mismatch"
        }

        $process = Start-Process -FilePath $temp -ArgumentList "/S /D=$installRoot" -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "Tesseract installer failed with exit code $($process.ExitCode)"
        }
        $runtimeSource = $installRoot
    }
    if (-not (Test-Path (Join-Path $runtimeSource "tesseract.exe"))) {
        throw "Tesseract runtime was not produced by the verified installer"
    }

    # Chỉ đóng gói CLI runtime; bỏ tool huấn luyện, tài liệu và uninstaller.
    Get-ChildItem -LiteralPath $target -File -ErrorAction SilentlyContinue | Remove-Item -Force
    Copy-Item -LiteralPath (Join-Path $runtimeSource "tesseract.exe") -Destination $target -Force
    Get-ChildItem -LiteralPath $runtimeSource -Filter "*.dll" -File |
        Copy-Item -Destination $target -Force
    Remove-Item -LiteralPath $modelRoot -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $modelRoot | Out-Null

    foreach ($model in $models) {
        $destination = Join-Path $modelRoot "$($model[0]).traineddata"
        Invoke-WebRequest -Uri $model[1] -OutFile $destination -UseBasicParsing
        if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $model[2]) {
            throw "OCR model $($model[0]) checksum mismatch"
        }
    }
    Remove-Item -LiteralPath (Join-Path $target "tessdata") -Recurse -Force -ErrorAction SilentlyContinue
    Copy-Item -LiteralPath $modelRoot -Destination (Join-Path $target "tessdata") -Recurse -Force
    Write-Host "Prepared bundled Tesseract OCR with vie+eng models."
} finally {
    Remove-Item -LiteralPath $temp -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $installRoot -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $modelRoot -Recurse -Force -ErrorAction SilentlyContinue
}
