[CmdletBinding()]
param(
    [ValidateSet("", "x64", "arm64")]
    [string]$Arch = "",

    [ValidateSet("nsis", "msi", "all", "none")]
    [string[]]$Bundle = @("nsis", "msi"),

    [switch]$Clean,
    [switch]$SkipDownloads,
    [switch]$SkipInstall,
    [switch]$CheckOnly,
    [switch]$OpenOutput,
    [switch]$VerboseBuild
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Ok {
    param([string]$Message)
    Write-Host "OK  $Message" -ForegroundColor Green
}

function Write-Warn {
    param([string]$Message)
    Write-Host "WARN $Message" -ForegroundColor Yellow
}

function Resolve-RepoRoot {
    $scriptDir = Split-Path -Parent $MyInvocation.ScriptName
    return (Resolve-Path (Join-Path $scriptDir "..")).Path
}

function Require-Command {
    param(
        [string]$Name,
        [string]$InstallHint
    )

    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if (-not $command) {
        throw "Missing required command '$Name'. $InstallHint"
    }

    return $command
}

function Invoke-Native {
    param(
        [string]$Name,
        [string]$FilePath,
        [string[]]$Arguments,
        [string]$WorkingDirectory
    )

    Write-Step $Name
    Write-Host "PS> $FilePath $($Arguments -join ' ')" -ForegroundColor DarkGray

    Push-Location $WorkingDirectory
    try {
        $global:LASTEXITCODE = 0
        & $FilePath @Arguments
        $exitCode = $global:LASTEXITCODE
        if ($exitCode -ne 0) {
            throw "$Name failed with exit code $exitCode."
        }
    }
    finally {
        Pop-Location
    }
}

function Test-CargoTauri {
    $global:LASTEXITCODE = 0
    $list = & cargo --list 2>$null
    if ($global:LASTEXITCODE -ne 0) {
        return $false
    }

    return (($list | Select-String -Pattern "^\s+tauri\s" -Quiet) -eq $true)
}

function Resolve-TauriRunner {
    $tauriNames = @("tauri.cmd", "tauri.exe", "tauri")
    foreach ($name in $tauriNames) {
        $command = Get-Command $name -ErrorAction SilentlyContinue
        if ($command) {
            return @{
                File = $command.Source
                PrefixArgs = @()
                Display = $command.Source
            }
        }
    }

    if (Test-CargoTauri) {
        $cargo = Require-Command "cargo" "Install Rust from https://rustup.rs/."
        return @{
            File = $cargo.Source
            PrefixArgs = @("tauri")
            Display = "$($cargo.Source) tauri"
        }
    }

    return $null
}

function Ensure-TauriRunner {
    param([bool]$AllowInstall)

    $runner = Resolve-TauriRunner
    if ($runner) {
        return $runner
    }

    if (-not $AllowInstall) {
        throw "Tauri CLI was not found. Install it with: cargo install tauri-cli --version `"^2.0.0`" --locked"
    }

    $cargo = Require-Command "cargo" "Install Rust from https://rustup.rs/."
    Invoke-Native `
        -Name "Install Tauri CLI" `
        -FilePath $cargo.Source `
        -Arguments @("install", "tauri-cli", "--version", "^2.0.0", "--locked") `
        -WorkingDirectory $RootDir

    $runner = Resolve-TauriRunner
    if (-not $runner) {
        throw "Tauri CLI install finished, but the 'tauri' runner is still unavailable. Restart the terminal and try again."
    }

    return $runner
}

function Get-TargetTriple {
    param([string]$RequestedArch)

    switch ($RequestedArch) {
        "x64" { return "x86_64-pc-windows-msvc" }
        "arm64" { return "aarch64-pc-windows-msvc" }
        default { return "" }
    }
}

function Get-FfmpegArch {
    param([string]$RequestedArch)

    if ($RequestedArch) {
        return $RequestedArch
    }

    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
        return "arm64"
    }

    return "x64"
}

function Test-RequiredFiles {
    param(
        [string]$ModelsDir,
        [string]$FfmpegDir,
        [string]$FfmpegArch
    )

    $models = @(
        "tokenizer.json",
        "text_model.onnx",
        "vision_model.onnx",
        "det_500m.onnx",
        "w600k_mbf.onnx"
    )

    $missing = @()
    foreach ($model in $models) {
        $path = Join-Path $ModelsDir $model
        if (-not (Test-Path $path)) {
            $missing += $path
        }
    }

    $ffmpegSuffix = if ($FfmpegArch -eq "arm64") {
        "aarch64-pc-windows-msvc.exe"
    }
    else {
        "x86_64-pc-windows-msvc.exe"
    }

    foreach ($tool in @("ffmpeg", "ffprobe")) {
        $path = Join-Path $FfmpegDir "$tool-$ffmpegSuffix"
        if (-not (Test-Path $path)) {
            $missing += $path
        }
    }

    return $missing
}

function Remove-ReleaseArtifacts {
    param([string]$ReleaseDir)

    $resolvedReleaseDir = (Resolve-Path $ReleaseDir -ErrorAction SilentlyContinue)
    if (-not $resolvedReleaseDir) {
        return
    }

    $releaseRoot = $resolvedReleaseDir.Path.TrimEnd("\")
    $targets = @(
        (Join-Path $releaseRoot "PicAiPic.exe"),
        (Join-Path $releaseRoot "PicAiPic.pdb"),
        (Join-Path $releaseRoot "bundle")
    )

    foreach ($target in $targets) {
        if (-not (Test-Path $target)) {
            continue
        }

        $resolvedTarget = (Resolve-Path $target).Path
        if (-not $resolvedTarget.StartsWith($releaseRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove path outside release directory: $resolvedTarget"
        }

        Remove-Item -LiteralPath $resolvedTarget -Recurse -Force
        Write-Ok "Removed $resolvedTarget"
    }
}

function Stop-RunningReleaseExe {
    param([string]$ExePath)

    $resolvedExePath = [System.IO.Path]::GetFullPath($ExePath)
    $running = @(
        Get-Process -Name "PicAiPic" -ErrorAction SilentlyContinue |
            Where-Object {
                try {
                    $_.Path -and ([System.IO.Path]::GetFullPath($_.Path) -eq $resolvedExePath)
                }
                catch {
                    $false
                }
            }
    )

    if ($running.Count -eq 0) {
        return
    }

    Write-Step "Closing running release EXE"
    foreach ($process in $running) {
        Write-Warn "Stopping PID $($process.Id): $($process.Path)"
        try {
            if ($process.MainWindowHandle -ne 0) {
                $null = $process.CloseMainWindow()
                if ($process.WaitForExit(5000)) {
                    Write-Ok "Closed PID $($process.Id)"
                    continue
                }
            }

            Stop-Process -Id $process.Id -Force -ErrorAction Stop
            $process.WaitForExit(5000)
            Write-Ok "Stopped PID $($process.Id)"
        }
        catch {
            throw "Failed to stop running release executable '$($process.Path)' (PID $($process.Id)): $($_.Exception.Message)"
        }
    }
}

function New-LocalTauriConfig {
    param(
        [string]$OutputPath
    )

    # Local config overrides only the build commands. We intentionally do NOT
    # override bundle.createUpdaterArtifacts here — it stays true (from
    # tauri.conf.json) so the build produces signed updater artifacts
    # (.sig + latest.json). The signing key is read from the
    # TAURI_SIGNING_PRIVATE_KEY env var; if unset, the build fails fast.
    $config = [ordered]@{
        build = [ordered]@{
            beforeBuildCommand = "pnpm --dir src-vite build"
            beforeDevCommand = "pnpm --dir src-vite dev"
        }
    }

    $parent = Split-Path -Parent $OutputPath
    if (-not (Test-Path $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }

    $config | ConvertTo-Json -Depth 8 | Set-Content -Path $OutputPath -Encoding UTF8
}

function Get-BuildArtifacts {
    param(
        [string]$ReleaseDir,
        [datetime]$Since
    )

    $paths = @()
    $exe = Join-Path $ReleaseDir "PicAiPic.exe"
    if (Test-Path $exe) {
        $paths += Get-Item $exe
    }

    $bundleDir = Join-Path $ReleaseDir "bundle"
    if (Test-Path $bundleDir) {
        $paths += Get-ChildItem $bundleDir -Recurse -File |
            Where-Object { $_.Extension -in @(".exe", ".msi") }
    }

    return $paths |
        Where-Object { $_.LastWriteTime -ge $Since } |
        Sort-Object FullName -Unique
}

function Show-Artifacts {
    param([object[]]$Artifacts)

    if (-not $Artifacts -or $Artifacts.Count -eq 0) {
        Write-Warn "No PicAiPic.exe, NSIS installer, or MSI installer was found."
        return
    }

    Write-Step "Build outputs"
    $rows = foreach ($artifact in $Artifacts) {
        $hash = Get-FileHash -Algorithm SHA256 -LiteralPath $artifact.FullName
        [pscustomobject]@{
            Path = $artifact.FullName
            SizeMB = "{0:N2}" -f ($artifact.Length / 1MB)
            LastWriteTime = $artifact.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss")
            SHA256 = $hash.Hash
        }
    }

    $rows | Format-Table -AutoSize
}

$RootDir = Resolve-RepoRoot
$FrontendDir = Join-Path $RootDir "src-vite"
$TauriDir = Join-Path $RootDir "src-tauri"
$ReleaseDir = Join-Path $TauriDir "target\release"
$ModelsDir = Join-Path $TauriDir "resources\models"
$FfmpegDir = Join-Path $TauriDir "resources\ffmpeg"
$BuildScratchDir = Join-Path $TauriDir "target\package-windows"
$LocalConfigPath = Join-Path $BuildScratchDir "tauri.local.conf.json"
$TargetTriple = Get-TargetTriple $Arch
$FfmpegArch = Get-FfmpegArch $Arch

# Load the updater signing key if not already set in the environment.
# The key file is gitignored and lives at the repo root; CI should set
# TAURI_SIGNING_PRIVATE_KEY directly instead.
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    $updaterKeyFile = Join-Path $RootDir "picaipic-updater-key.key"
    if (Test-Path $updaterKeyFile) {
        $env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content -LiteralPath $updaterKeyFile -Raw).Trim()
        Write-Ok "Loaded updater signing key from $updaterKeyFile"
    }
    else {
        Write-Warn "No updater signing key found. Set TAURI_SIGNING_PRIVATE_KEY or place picaipic-updater-key.key at the repo root. Updater artifacts will not be signed."
    }
}

Write-Host "PicAiPic Windows package script" -ForegroundColor White
Write-Host "Project: $RootDir"

$isWindowsHost = [System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT
if (-not $isWindowsHost) {
    throw "This script packages the Windows desktop app and must be run on Windows."
}

if (-not (Test-Path (Join-Path $TauriDir "tauri.conf.json"))) {
    throw "Cannot find src-tauri\tauri.conf.json under $RootDir."
}

Write-Step "Checking tools"
$nodeCommand = Require-Command "node" "Install Node.js 20 or newer."
$pnpmCommand = Require-Command "pnpm" "Install pnpm, or enable it with Corepack."
$cargoCommand = Require-Command "cargo" "Install Rust from https://rustup.rs/."
Write-Ok "Node: $($nodeCommand.Source)"
Write-Ok "pnpm: $($pnpmCommand.Source)"
Write-Ok "Cargo: $($cargoCommand.Source)"
$TauriRunner = Ensure-TauriRunner -AllowInstall:(!$SkipInstall -and !$CheckOnly)
Write-Ok "Tauri runner: $($TauriRunner.Display)"

Write-Step "Checking host bundled resources"
$missingFiles = @(Test-RequiredFiles -ModelsDir $ModelsDir -FfmpegDir $FfmpegDir -FfmpegArch $FfmpegArch)
if ($missingFiles.Count -eq 0) {
    Write-Ok "Host models and FFmpeg sidecars are present. External AI plugins are not bundled."
}
elseif ($SkipDownloads -or $CheckOnly) {
    Write-Warn "Missing host bundled resources:"
    $missingFiles | ForEach-Object { Write-Host "  $_" }
    if ($CheckOnly) {
        Write-Warn "Run without -CheckOnly to download missing resources automatically."
    }
    else {
        throw "Missing host bundled resources and -SkipDownloads was set."
    }
}
else {
    Invoke-Native `
        -Name "Download AI models if needed" `
        -FilePath "powershell" `
        -Arguments @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", (Join-Path $RootDir "scripts\download_models.ps1")) `
        -WorkingDirectory $RootDir

    Invoke-Native `
        -Name "Download FFmpeg sidecars if needed" `
        -FilePath "powershell" `
        -Arguments @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", (Join-Path $RootDir "scripts\download_ffmpeg_sidecar.ps1"), "-Arch", $FfmpegArch) `
        -WorkingDirectory $RootDir
}

if ($CheckOnly) {
    Write-Step "Check complete"
    Write-Host "Run .\build-exe.bat or .\scripts\package_windows.ps1 to build the EXE and installers."
    exit 0
}

if (-not (Test-Path (Join-Path $FrontendDir "node_modules"))) {
    if ($SkipInstall) {
        throw "src-vite\node_modules is missing and -SkipInstall was set."
    }

    Invoke-Native `
        -Name "Install frontend dependencies" `
        -FilePath "pnpm" `
        -Arguments @("--dir", $FrontendDir, "install") `
        -WorkingDirectory $RootDir
}
else {
    Write-Ok "Frontend dependencies already installed."
}

Stop-RunningReleaseExe -ExePath (Join-Path $ReleaseDir "PicAiPic.exe")

if ($Clean) {
    Write-Step "Cleaning old release artifacts"
    Remove-ReleaseArtifacts -ReleaseDir $ReleaseDir
}

New-LocalTauriConfig -OutputPath $LocalConfigPath

$buildArgs = @()
$buildArgs += $TauriRunner.PrefixArgs
# Note: do NOT pass --no-sign here. That flag skips BOTH Windows code
# signing AND updater signing. We have no Authenticode certificate, but
# Tauri handles that gracefully (it only signs if signtool is configured).
# Removing --no-sign lets the updater artifacts (.sig + latest.json) be
# produced, signed with TAURI_SIGNING_PRIVATE_KEY.
$buildArgs += @("build", "--ci", "--config", $LocalConfigPath)

if ($VerboseBuild) {
    $buildArgs += "--verbose"
}

if ($TargetTriple) {
    $buildArgs += @("--target", $TargetTriple)
}

if ($Bundle -contains "none") {
    $buildArgs += "--no-bundle"
}
elseif (-not ($Bundle -contains "all")) {
    $buildArgs += @("--bundles", (($Bundle | Select-Object -Unique) -join ","))
}

$buildStartTime = Get-Date
Invoke-Native `
    -Name "Build PicAiPic release package" `
    -FilePath $TauriRunner.File `
    -Arguments $buildArgs `
    -WorkingDirectory $TauriDir

$artifacts = @(Get-BuildArtifacts -ReleaseDir $ReleaseDir -Since $buildStartTime.AddSeconds(-2))
Show-Artifacts -Artifacts $artifacts

if ($OpenOutput) {
    $bundleDir = Join-Path $ReleaseDir "bundle"
    if (Test-Path $bundleDir) {
        Start-Process explorer.exe $bundleDir
    }
    else {
        Start-Process explorer.exe $ReleaseDir
    }
}

Write-Step "Done"
Write-Host "Main EXE: $(Join-Path $ReleaseDir "PicAiPic.exe")"
Write-Host "Installers: $(Join-Path $ReleaseDir "bundle")"
