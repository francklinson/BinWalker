# BinWalker Build Script
# Usage: .\build.ps1 [command]
# Commands: dev, build, clean, check, run

param(
    [Parameter(Position=0)]
    [ValidateSet("dev", "build", "clean", "check", "run")]
    [string]$Command = "check"
)

$ErrorActionPreference = "Stop"

function Write-Header {
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "    BinWalker Firmware Analysis Tool" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host ""
}

function Test-Command {
    param([string]$Name)
    $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Check-Environment {
    Write-Host "Checking development environment..." -ForegroundColor Yellow
    
    $envOk = $true
    
    # Check Node.js
    if (Test-Command "node") {
        $nodeVersion = node --version
        Write-Host "  [OK] Node.js: $nodeVersion" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] Node.js not installed" -ForegroundColor Red
        Write-Host "       Install: winget install OpenJS.NodeJS.LTS" -ForegroundColor Gray
        $envOk = $false
    }
    
    # Check npm
    if (Test-Command "npm") {
        $npmVersion = npm --version
        Write-Host "  [OK] npm: $npmVersion" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] npm not installed" -ForegroundColor Red
        $envOk = $false
    }
    
    # Check Rust
    if (Test-Command "rustc") {
        $rustVersion = rustc --version
        Write-Host "  [OK] Rust: $rustVersion" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] Rust not installed" -ForegroundColor Red
        Write-Host "       Install: winget install Rustlang.Rustup" -ForegroundColor Gray
        $envOk = $false
    }
    
    # Check Cargo
    if (Test-Command "cargo") {
        $cargoVersion = cargo --version
        Write-Host "  [OK] Cargo: $cargoVersion" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] Cargo not installed" -ForegroundColor Red
        $envOk = $false
    }
    
    # Check WebView2
    $webView2Path = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    if (Test-Path $webView2Path) {
        Write-Host "  [OK] WebView2 Runtime installed" -ForegroundColor Green
    } else {
        Write-Host "  [WARN] WebView2 Runtime may not be installed" -ForegroundColor Yellow
        Write-Host "         Download: https://developer.microsoft.com/en-us/microsoft-edge/webview2/" -ForegroundColor Gray
    }
    
    Write-Host ""
    
    if (-not $envOk) {
        Write-Host "Environment check failed. Please install missing components." -ForegroundColor Red
        exit 1
    }
    
    Write-Host "Environment check passed!" -ForegroundColor Green
    Write-Host ""
}

function Install-Dependencies {
    Write-Host "Installing npm dependencies..." -ForegroundColor Yellow
    
    if (-not (Test-Path "node_modules")) {
        npm install
        if ($LASTEXITCODE -ne 0) {
            Write-Host "npm dependency installation failed" -ForegroundColor Red
            exit 1
        }
    } else {
        Write-Host "  node_modules exists, skipping install" -ForegroundColor Gray
    }
    
    Write-Host "npm dependencies installed!" -ForegroundColor Green
    Write-Host ""
}

function Invoke-Dev {
    Write-Host "Starting development mode..." -ForegroundColor Yellow
    Write-Host "  Press Ctrl+C to stop" -ForegroundColor Gray
    Write-Host ""
    
    Install-Dependencies
    
    npm run tauri dev
}

function Invoke-Build {
    Write-Host "Building production version..." -ForegroundColor Yellow
    
    Install-Dependencies
    
    Write-Host "Compiling..." -ForegroundColor Yellow
    npm run tauri build
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Host "========================================" -ForegroundColor Green
        Write-Host "  Build successful!" -ForegroundColor Green
        Write-Host "========================================" -ForegroundColor Green
        Write-Host ""
        Write-Host "Output location:" -ForegroundColor Cyan
        Write-Host "  src-tauri\target\release\BinWalker.exe" -ForegroundColor White
        Write-Host ""
        Write-Host "Installer location:" -ForegroundColor Cyan
        Write-Host "  src-tauri\target\release\bundle\nsis\" -ForegroundColor White
        Write-Host ""
    } else {
        Write-Host "Build failed" -ForegroundColor Red
        exit 1
    }
}

function Invoke-Clean {
    Write-Host "Cleaning build cache..." -ForegroundColor Yellow
    
    if (Test-Path "src-tauri\target") {
        Remove-Item -Recurse -Force "src-tauri\target"
        Write-Host "  Cleaned Rust build cache" -ForegroundColor Green
    }
    
    if (Test-Path "dist") {
        Remove-Item -Recurse -Force "dist"
        Write-Host "  Cleaned frontend build cache" -ForegroundColor Green
    }
    
    if (Test-Path "node_modules") {
        $confirm = Read-Host "Delete node_modules? (y/N)"
        if ($confirm -eq "y" -or $confirm -eq "Y") {
            Remove-Item -Recurse -Force "node_modules"
            Write-Host "  Deleted node_modules" -ForegroundColor Green
        }
    }
    
    Write-Host "Clean complete!" -ForegroundColor Green
}

function Invoke-Run {
    $exePath = "src-tauri\target\release\BinWalker.exe"
    
    if (Test-Path $exePath) {
        Write-Host "Starting BinWalker..." -ForegroundColor Yellow
        Write-Host ""
        Start-Process $exePath
    } else {
        Write-Host "Executable not found: $exePath" -ForegroundColor Red
        Write-Host "Please run: .\build.ps1 build" -ForegroundColor Gray
        exit 1
    }
}

# Main program
Write-Header

switch ($Command) {
    "check" { Check-Environment }
    "dev" { Invoke-Dev }
    "build" { Invoke-Build }
    "clean" { Invoke-Clean }
    "run" { Invoke-Run }
    default { 
        Write-Host "Usage: .\build.ps1 [command]" -ForegroundColor Yellow
        Write-Host ""
        Write-Host "Commands:" -ForegroundColor Cyan
        Write-Host "  check  - Check development environment (default)" -ForegroundColor White
        Write-Host "  dev    - Start development mode (hot reload)" -ForegroundColor White
        Write-Host "  build  - Build production version" -ForegroundColor White
        Write-Host "  run    - Run built program" -ForegroundColor White
        Write-Host "  clean  - Clean build cache" -ForegroundColor White
        Write-Host ""
    }
}
