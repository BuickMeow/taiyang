# Taiyang Plugin Windows Deploy Script
# Deploys taiyang and taiyang16 VST3 + CLAP plugins locally

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ReleaseDir = "$ProjectRoot\target\release"

$SystemVst3Dir = "$env:COMMONPROGRAMFILES\VST3"
$SystemClapDir = "$env:COMMONPROGRAMFILES\CLAP"

$UserVst3Dir = "$env:LOCALAPPDATA\Programs\Common\VST3"
$UserClapDir = "$env:LOCALAPPDATA\Programs\Common\CLAP"

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

function Deploy-Vst3 {
    param([string]$PluginName, [string]$DestDir)
    $Path = "$DestDir\$PluginName.vst3"
    New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
    Copy-Item "$ReleaseDir\$PluginName.dll" "$Path" -Force
    Write-Host "  VST3 deployed: $Path" -ForegroundColor Green
}

function Deploy-Clap {
    param([string]$PluginName, [string]$DestDir)
    New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
    Copy-Item "$ReleaseDir\$PluginName.dll" "$DestDir\$PluginName.clap" -Force
    Write-Host "  CLAP deployed: $DestDir\$PluginName.clap" -ForegroundColor Green
}

function Deploy-Plugin {
    param([string]$PluginName, [string]$Vst3Dir, [string]$ClapDir)
    Write-Host "Deploying $PluginName..." -ForegroundColor Yellow
    Deploy-Vst3 -PluginName $PluginName -DestDir $Vst3Dir
    Deploy-Clap -PluginName $PluginName -DestDir $ClapDir
}

Write-Host "Select deploy target:" -ForegroundColor Cyan
Write-Host "  [1] User directory (recommended, no admin)" -ForegroundColor White
Write-Host "  [2] System directory (requires admin PowerShell)" -ForegroundColor White
Write-Host "  [3] Build only, skip install" -ForegroundColor White
$choice = Read-Host "Enter option (1/2/3)"

switch ($choice) {
    "1" {
        $Vst3Dir = $UserVst3Dir
        $ClapDir = $UserClapDir
        Build-Project
    }
    "2" {
        $Vst3Dir = $SystemVst3Dir
        $ClapDir = $SystemClapDir
        Build-Project
    }
    "3" {
        Write-Host "Skipping install, build only..." -ForegroundColor Yellow
        Build-Project
        exit 0
    }
    default {
        Write-Host "Invalid option, exiting" -ForegroundColor Red
        exit 1
    }
}

Deploy-Plugin -PluginName "taiyang" -Vst3Dir $Vst3Dir -ClapDir $ClapDir
Deploy-Plugin -PluginName "taiyang16" -Vst3Dir $Vst3Dir -ClapDir $ClapDir

Write-Host ""
Write-Host "All done! Rescan plugins in your DAW." -ForegroundColor Green
Write-Host "Hint: Use 'Scan Now' or 'Reset Plugin Catalog' in your DAW" -ForegroundColor Yellow
Write-Host ""
Write-Host "Plugins:" -ForegroundColor Cyan
Write-Host "  Taiyang   - Mono-channel SoundFont synth" -ForegroundColor White
Write-Host "  Taiyang16 - 16-channel SoundFont synth (full MIDI)" -ForegroundColor White
