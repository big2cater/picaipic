[CmdletBinding()]
param(
    [switch]$IncludeStress,
    [switch]$SkipFrontendBuild,
    [switch]$SkipCargoCheck,
    [switch]$SkipCargoFmt,
    [switch]$SkipPythonCompile,
    [switch]$FastStress
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

function Write-Fail {
    param([string]$Message)
    Write-Host "ERR $Message" -ForegroundColor Red
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

function Invoke-Check {
    param(
        [string]$Name,
        [string]$FilePath,
        [string[]]$Arguments,
        [string]$WorkingDirectory
    )

    Write-Step $Name
    Write-Host "PS> $FilePath $($Arguments -join ' ')" -ForegroundColor DarkGray
    $started = Get-Date
    Push-Location $WorkingDirectory
    try {
        $global:LASTEXITCODE = 0
        & $FilePath @Arguments
        $exitCode = $global:LASTEXITCODE
        if ($exitCode -ne 0) {
            throw "$Name failed with exit code $exitCode."
        }
        $elapsed = (Get-Date) - $started
        Write-Ok "$Name passed in $([math]::Round($elapsed.TotalSeconds, 1))s."
    }
    finally {
        Pop-Location
    }
}

function Test-JsonFile {
    param([string]$Path)

    $fullPath = (Resolve-Path $Path).Path
    # Windows PowerShell 5.1's ConvertFrom-Json throws ArgumentException
    # ("Invalid object passed in, ':' or '}' expected") on these manifests even
    # though they are valid JSON, which aborted the whole check on Windows.
    # Node is already required above, so validate with it instead.
    $global:LASTEXITCODE = 0
    & node -e "JSON.parse(require('fs').readFileSync(process.argv[1], 'utf8'))" $fullPath | Out-Null
    if ($global:LASTEXITCODE -ne 0) {
        throw "Invalid JSON: $fullPath"
    }
    Write-Ok "JSON valid: $fullPath"
}

$RootDir = Resolve-RepoRoot
$TauriDir = Join-Path $RootDir "src-tauri"
$FrontendDir = Join-Path $RootDir "src-vite"
$SalutPluginDir = Join-Path $RootDir "plugins\picai-salut-color"
$NafnetPluginDir = Join-Path $RootDir "plugins\picai-nafnet-restore"

Write-Host "PicAiPic plugin host check" -ForegroundColor White
Write-Host "Project: $RootDir"

Write-Step "Checking tools"
$nodeCommand = Require-Command "node" "Install Node.js 20 or newer."
$pnpmCommand = Require-Command "pnpm" "Install pnpm, or enable it with Corepack."
$cargoCommand = Require-Command "cargo" "Install Rust from https://rustup.rs/."
$pythonCommand = Require-Command "python" "Install Python or add it to PATH."
Write-Ok "Node: $($nodeCommand.Source)"
Write-Ok "pnpm: $($pnpmCommand.Source)"
Write-Ok "Cargo: $($cargoCommand.Source)"
Write-Ok "Python: $($pythonCommand.Source)"

try {
    Write-Step "Validating plugin manifests"
    Test-JsonFile -Path (Join-Path $SalutPluginDir "picaipic.plugin.json")
    Test-JsonFile -Path (Join-Path $NafnetPluginDir "picaipic.plugin.json")

    if (-not $SkipCargoFmt) {
        Invoke-Check `
            -Name "Rust format check" `
            -FilePath $cargoCommand.Source `
            -Arguments @("fmt", "--manifest-path", (Join-Path $TauriDir "Cargo.toml"), "--check") `
            -WorkingDirectory $RootDir
    }

    if (-not $SkipCargoCheck) {
        Invoke-Check `
            -Name "Rust cargo check" `
            -FilePath $cargoCommand.Source `
            -Arguments @("check", "--manifest-path", (Join-Path $TauriDir "Cargo.toml")) `
            -WorkingDirectory $RootDir
    }

    if (-not $SkipFrontendBuild) {
        Invoke-Check `
            -Name "Frontend production build" `
            -FilePath $pnpmCommand.Source `
            -Arguments @("--dir", $FrontendDir, "build") `
            -WorkingDirectory $RootDir
    }

    if (-not $SkipPythonCompile) {
        Invoke-Check `
            -Name "SA-LUT backend Python compile" `
            -FilePath $pythonCommand.Source `
            -Arguments @(
                "-m",
                "py_compile",
                (Join-Path $SalutPluginDir "backend\main.py"),
                (Join-Path $SalutPluginDir "backend\salut_adapter.py")
            ) `
            -WorkingDirectory $RootDir

        Invoke-Check `
            -Name "NAFNet backend Python compile" `
            -FilePath $pythonCommand.Source `
            -Arguments @(
                "-m",
                "py_compile",
                (Join-Path $NafnetPluginDir "backend\main.py"),
                (Join-Path $NafnetPluginDir "backend\nafnet_adapter.py"),
                (Join-Path $NafnetPluginDir "backend\denoiser.py")
            ) `
            -WorkingDirectory $RootDir
    }

    if ($IncludeStress) {
        $salutAsyncArgs = @("scripts\stress_salut_async.py", "--tasks", "8", "--duration-ms", "300", "--cancel-every", "3")
        $salutHttpArgs = @("scripts\stress_salut_http.py", "--tasks", "6", "--duration-ms", "250", "--cancel-every", "3")
        $nafnetHttpArgs = @("scripts\stress_nafnet_http.py", "--tasks", "4", "--duration-ms", "120", "--cancel-every", "2")

        if ($FastStress) {
            $salutAsyncArgs = @("scripts\stress_salut_async.py", "--tasks", "4", "--duration-ms", "150", "--cancel-every", "2")
            $salutHttpArgs = @("scripts\stress_salut_http.py", "--tasks", "4", "--duration-ms", "150", "--cancel-every", "2")
            $nafnetHttpArgs = @("scripts\stress_nafnet_http.py", "--tasks", "3", "--duration-ms", "100", "--cancel-every", "3")
        }

        Invoke-Check `
            -Name "SA-LUT async mock stress" `
            -FilePath $pythonCommand.Source `
            -Arguments $salutAsyncArgs `
            -WorkingDirectory $RootDir

        Invoke-Check `
            -Name "SA-LUT HTTP mock stress" `
            -FilePath $pythonCommand.Source `
            -Arguments $salutHttpArgs `
            -WorkingDirectory $RootDir

        Invoke-Check `
            -Name "NAFNet HTTP mock stress" `
            -FilePath $pythonCommand.Source `
            -Arguments $nafnetHttpArgs `
            -WorkingDirectory $RootDir
    }
    else {
        Write-Step "Stress tests skipped"
        Write-Host "Run with -IncludeStress to exercise mock async/HTTP plugin task plumbing."
    }

    Write-Step "Done"
    Write-Host "All selected checks passed." -ForegroundColor Green
}
catch {
    Write-Fail $_.Exception.Message
    throw
}
