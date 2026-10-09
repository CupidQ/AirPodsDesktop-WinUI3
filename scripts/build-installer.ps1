param(
    [string]$IsccPath,
    [string]$VcRedistDir
)

$ErrorActionPreference = 'Stop'
$packageRoot = Split-Path -Parent $PSScriptRoot
Set-Location $packageRoot

if (-not $IsccPath) {
    $compilerCommand = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($compilerCommand) { $IsccPath = $compilerCommand.Source }
    foreach ($compilerCandidate in @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 7\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
    )) {
        if (-not $IsccPath -and (Test-Path -LiteralPath $compilerCandidate)) { $IsccPath = $compilerCandidate }
    }
}
if (-not $IsccPath -or -not (Test-Path -LiteralPath $IsccPath)) {
    throw 'Install Inno Setup 6.5+ or pass -IsccPath to ISCC.exe.'
}

if (-not $VcRedistDir) {
    if ($env:VCToolsRedistDir) {
        $VcRedistDir = Join-Path $env:VCToolsRedistDir 'x64\Microsoft.VC143.CRT'
    } else {
        $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
        if (Test-Path -LiteralPath $vswhere) {
            $vsInstall = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
            if ($vsInstall) {
                $redistBase = Join-Path $vsInstall 'VC\Redist\MSVC'
                $redistVersion = Get-ChildItem -LiteralPath $redistBase -Directory |
                    Where-Object { $_.Name -match '^\d+\.\d+\.\d+$' } |
                    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
                if ($redistVersion) { $VcRedistDir = Join-Path $redistVersion.FullName 'x64\Microsoft.VC143.CRT' }
            }
        }
    }
}
if (-not $VcRedistDir -or -not (Test-Path -LiteralPath (Join-Path $VcRedistDir 'vcruntime140.dll'))) {
    throw 'Visual C++ x64 app-local redistributables not found. Pass -VcRedistDir to Microsoft.VC143.CRT.'
}

. (Join-Path $PSScriptRoot 'build.ps1')
$dotnet = Find-DotNet
$publishDir = Join-Path $packageRoot 'dist\publish'
# Remove only the generated staging directory so outdated files never enter the installer.
if (Test-Path -LiteralPath $publishDir) {
    $resolvedStage = (Resolve-Path -LiteralPath $publishDir).Path
    if ($resolvedStage -ne (Join-Path $packageRoot 'dist\publish')) { throw 'Unexpected staging path.' }
    Remove-Item -LiteralPath $resolvedStage -Recurse -Force
}
& $dotnet publish (Join-Path $packageRoot 'app\AirPodsDesktop\AirPodsDesktop.csproj') `
    -c Release -r win-x64 --self-contained true -p:Platform=x64 `
    -p:WindowsAppSDKSelfContained=true -p:PublishTrimmed=false -o $publishDir /nr:false
if ($LASTEXITCODE -ne 0) { throw 'Self-contained publish failed.' }

Copy-Item -Path (Join-Path $VcRedistDir '*.dll') -Destination $publishDir -Force
Copy-Item -LiteralPath (Join-Path $packageRoot 'LICENSE') -Destination $publishDir
Copy-Item -LiteralPath (Join-Path $packageRoot 'README.md') -Destination $publishDir
Copy-Item -LiteralPath (Join-Path $packageRoot 'docs') -Destination $publishDir -Recurse

$runtimeConfig = Get-Content -LiteralPath (Join-Path $publishDir 'AirPodsDesktop.runtimeconfig.json') -Raw | ConvertFrom-Json
$runtimeVersion = $runtimeConfig.runtimeOptions.includedFrameworks | Where-Object name -eq 'Microsoft.NETCore.App' | Select-Object -ExpandProperty version
if (-not $runtimeVersion -or $runtimeConfig.runtimeOptions.framework) { throw 'The .NET runtime was not bundled.' }
$packageCache = if ($env:NUGET_PACKAGES) { $env:NUGET_PACKAGES } else { Join-Path $env:USERPROFILE '.nuget\packages' }
$noticeDir = Join-Path $publishDir 'ThirdPartyNotices'
New-Item -ItemType Directory -Path $noticeDir -Force | Out-Null
foreach ($noticePackage in @(
    @{ Prefix = 'dotnet'; Path = "microsoft.netcore.app.runtime.win-x64\$runtimeVersion"; Files = @('LICENSE.TXT', 'THIRD-PARTY-NOTICES.TXT') },
    @{ Prefix = 'windowsappsdk'; Path = 'microsoft.windowsappsdk\1.5.240802000'; Files = @('license.txt', 'NOTICE.txt') }
)) {
    foreach ($noticeFile in $noticePackage.Files) {
        Copy-Item -LiteralPath (Join-Path $packageCache (Join-Path $noticePackage.Path $noticeFile)) `
            -Destination (Join-Path $noticeDir ($noticePackage.Prefix + '-' + $noticeFile))
    }
}

foreach ($requiredFile in @('AirPodsDesktop.exe', 'apd_core.dll', 'Microsoft.UI.Xaml.dll', 'coreclr.dll', 'hostfxr.dll', 'vcruntime140.dll', 'msvcp140.dll')) {
    if (-not (Test-Path -LiteralPath (Join-Path $publishDir $requiredFile))) { throw "Missing payload: $requiredFile" }
}
[xml]$appProject = Get-Content -LiteralPath (Join-Path $packageRoot 'app\AirPodsDesktop\AirPodsDesktop.csproj')
$appVersion = [string]$appProject.Project.PropertyGroup.Version
& $IsccPath '/Qp' "/DAppVersion=$appVersion" "/DPublishDir=$publishDir" (Join-Path $PSScriptRoot 'installer\AirPodsDesktop.iss')
if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
$setupFile = Join-Path $packageRoot "dist\AirPodsDesktop-WinUI3-$appVersion-Setup-x64.exe"
$setupHash = (Get-FileHash -LiteralPath $setupFile -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath (Join-Path $packageRoot 'dist\SHA256SUMS.txt') `
    -Value "$setupHash  $([IO.Path]::GetFileName($setupFile))" -Encoding ascii
Write-Host "Installer: $setupFile"
