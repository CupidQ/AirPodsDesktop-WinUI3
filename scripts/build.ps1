# Build AirPodsDesktop (Rust core + WinUI 3)
# Requires: Rust (msvc), .NET 8 SDK, VS Build Tools (C++), Windows 10/11 SDK

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Find-VcVars {
    $candidates = @(
        "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
        "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat",
        "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat",
        "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    )
    foreach ($c in $candidates) { if (Test-Path $c) { return $c } }
    return $null
}

function Find-DotNet {
    if ($env:DOTNET_ROOT -and (Test-Path "$env:DOTNET_ROOT\dotnet.exe")) {
        return "$env:DOTNET_ROOT\dotnet.exe"
    }
    $userSdk = Join-Path $env:USERPROFILE ".dotnet\dotnet.exe"
    if (Test-Path $userSdk) {
        return $userSdk
    }
    $cmd = Get-Command dotnet -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    throw "dotnet SDK not found. Install .NET 8 SDK."
}

# --- Ensure critical env vars (NuGet restore fails without PROGRAMFILES(X86)) ---
if (-not $env:USERPROFILE) { $env:USERPROFILE = "$env:HOMEDRIVE$env:HOMEPATH" }
if (-not [Environment]::GetEnvironmentVariable("PROGRAMFILES(X86)")) {
    [Environment]::SetEnvironmentVariable("PROGRAMFILES(X86)", "C:\Program Files (x86)", "Process")
}
if (-not [Environment]::GetEnvironmentVariable("PROGRAMFILES")) {
    [Environment]::SetEnvironmentVariable("PROGRAMFILES", "C:\Program Files", "Process")
}
$env:DOTNET_CLI_TELEMETRY_OPTOUT = "1"
$env:DOTNET_CLI_UI_LANGUAGE = "en"
if (-not $env:NUGET_PACKAGES) {
    $env:NUGET_PACKAGES = Join-Path $env:USERPROFILE ".nuget\packages"
}

Write-Host "==> Building Rust core (apd_core)..." -ForegroundColor Cyan
$vcvars = Find-VcVars
if ($vcvars) {
    cmd /c "`"$vcvars`" && set PATH=%USERPROFILE%\.cargo\bin;%PATH% && cd /d `"$Root`" && cargo build -p apd-core --release --locked"
} else {
    cargo build -p apd-core --release --locked
}
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

Write-Host "==> Running Rust unit tests..." -ForegroundColor Cyan
if ($vcvars) {
    cmd /c "`"$vcvars`" && set PATH=%USERPROFILE%\.cargo\bin;%PATH% && cd /d `"$Root`" && cargo test -p apd-core --locked"
} else {
    cargo test -p apd-core --locked
}
if ($LASTEXITCODE -ne 0) { throw "cargo tests failed" }

$rustDll = Join-Path $Root "target\release\apd_core.dll"
if (-not (Test-Path $rustDll)) { throw "apd_core.dll not produced" }

Write-Host "==> Building WinUI 3 app..." -ForegroundColor Cyan
$dotnet = Find-DotNet
$csproj = Join-Path $Root "app\AirPodsDesktop\AirPodsDesktop.csproj"
& $dotnet build $csproj -c Release -p:Platform=x64 /nr:false
if ($LASTEXITCODE -ne 0) { throw "dotnet build failed" }

$outDir = Join-Path $Root "app\AirPodsDesktop\bin\x64\Release\net8.0-windows10.0.19041.0"
Copy-Item $rustDll (Join-Path $outDir "apd_core.dll") -Force
Write-Host "==> Done. Output: $outDir" -ForegroundColor Green
Get-ChildItem $outDir | Where-Object { $_.Name -match '^(AirPodsDesktop|apd_core)\.(exe|dll)$' } | Format-Table Name, Length
