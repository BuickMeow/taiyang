# Taiyang Plugin Windows Packager
# Builds VST3 + CLAP plugins into a zip for distribution

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ReleaseDir = "$ProjectRoot\target\release"
$PackageDir = "$ProjectRoot\package"
$Version = "0.1.0"

function Build-Project {
    Write-Host "Building project..." -ForegroundColor Yellow
    Push-Location $ProjectRoot
    try {
        cargo build --release
        if ($LASTEXITCODE -ne 0) {
            throw "Build failed"
        }
    } finally {
        Pop-Location
    }
    Write-Host "Build OK" -ForegroundColor Green
}

function Package-Vst3 {
    param([string]$PluginName)
    $Dir = "$PackageDir\VST3"
    New-Item -ItemType Directory -Force -Path $Dir | Out-Null
    Copy-Item "$ReleaseDir\$PluginName.dll" "$Dir\$PluginName.vst3" -Force
    Write-Host "  VST3: $PluginName.vst3" -ForegroundColor Green
}

function Package-Clap {
    param([string]$PluginName)
    $Dir = "$PackageDir\CLAP"
    New-Item -ItemType Directory -Force -Path $Dir | Out-Null
    Copy-Item "$ReleaseDir\$PluginName.dll" "$Dir\$PluginName.clap" -Force
    Write-Host "  CLAP: $PluginName.clap" -ForegroundColor Green
}

function Create-Readme {
    $Text = @"
Taiyang SoundFont Synthesizer v$Version
======================================

Plugins included:
  - Taiyang.vst3    Mono-channel SoundFont synth (VST3)
  - Taiyang.clap    Mono-channel SoundFont synth (CLAP)
  - Taiyang16.vst3  16-channel SoundFont synth (VST3)
  - Taiyang16.clap  16-channel SoundFont synth (CLAP)

Install VST3:
  Copy .vst3 files to:
  C:\Program Files\Common Files\VST3\
  or
  %LOCALAPPDATA%\Programs\Common\VST3\

Install CLAP:
  Copy .clap files to:
  C:\Program Files\Common Files\CLAP\
  or
  %LOCALAPPDATA%\Programs\Common\CLAP\

Then rescan plugins in your DAW.

Requirements:
  - Windows 10/11 64-bit
  - DAW with VST3 or CLAP support

Homepage: https://space.bilibili.com/433246974
"@
    $Text | Out-File -Encoding UTF8 "$PackageDir\README.txt"
}

# Clean and create package dir
if (Test-Path $PackageDir) {
    Remove-Item -Recurse -Force $PackageDir
}
New-Item -ItemType Directory -Force -Path $PackageDir | Out-Null

Build-Project

Write-Host "Packaging..." -ForegroundColor Yellow

Package-Vst3 "taiyang"
Package-Clap "taiyang"
Package-Vst3 "taiyang16"
Package-Clap "taiyang16"

Create-Readme

$ZipName = "Taiyang-$Version-Windows-x64.zip"
$ZipPath = "$ProjectRoot\$ZipName"

if (Test-Path $ZipPath) {
    Remove-Item $ZipPath -Force
}

Compress-Archive -Path "$PackageDir\*" -DestinationPath $ZipPath

Write-Host ""
Write-Host "Done: $ZipName" -ForegroundColor Green
Write-Host "Location: $ZipPath" -ForegroundColor Cyan
Write-Host ""
Write-Host "Send this zip to your friend." -ForegroundColor Green

Remove-Item -Recurse -Force $PackageDir
