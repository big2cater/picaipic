[CmdletBinding(DefaultParameterSetName = "Path")]
param(
    [Parameter(ParameterSetName = "Path", Position = 0)]
    [string]$PluginPath,

    [Parameter(ParameterSetName = "All")]
    [switch]$All,

    [string]$OutputDir,
    [switch]$FailOnWarnings,
    [switch]$KeepStaging,
    [string]$SignKeyFile
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

function Test-PathInside {
    param(
        [string]$Path,
        [string]$Root
    )

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    $resolvedRoot = [System.IO.Path]::GetFullPath($Root).TrimEnd("\") + "\"
    return $resolvedPath.StartsWith($resolvedRoot, [System.StringComparison]::OrdinalIgnoreCase)
}

function Remove-DirectorySafe {
    param(
        [string]$Path,
        [string]$AllowedRoot
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }

    if (-not (Test-PathInside -Path $Path -Root $AllowedRoot)) {
        throw "Refusing to remove staging path outside allowed root: $Path"
    }

    Remove-Item -LiteralPath $Path -Recurse -Force
}

function ConvertTo-RelativePath {
    param(
        [string]$Path,
        [string]$Base
    )

    $pathUri = [System.Uri]([System.IO.Path]::GetFullPath($Path))
    $basePath = [System.IO.Path]::GetFullPath($Base).TrimEnd("\") + "\"
    $baseUri = [System.Uri]$basePath
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace("/", "\")
}

function Test-SafeRelativeCommand {
    param([string]$Command)

    if (-not $Command) {
        return $true
    }

    if ([System.IO.Path]::IsPathRooted($Command)) {
        return $false
    }

    $normalized = $Command.Replace("/", "\")
    if ($normalized -match '(^|\\)\.\.(\\|$)') {
        return $false
    }

    return $true
}

function Get-ManifestCommandWarnings {
    param(
        [object]$Manifest,
        [string]$PluginRoot
    )

    $warnings = @()

    $startCommand = Get-OptionalString $Manifest.entry "startCommand"
    if (-not (Test-SafeRelativeCommand $startCommand)) {
        $warnings += "entry.startCommand is not a safe relative path: $startCommand"
    }
    elseif ($startCommand -and -not (Test-Path -LiteralPath (Join-Path $PluginRoot $startCommand))) {
        $warnings += "entry.startCommand does not exist: $startCommand"
    }

    $stopCommand = Get-OptionalString $Manifest.entry "stopCommand"
    if (-not (Test-SafeRelativeCommand $stopCommand)) {
        $warnings += "entry.stopCommand is not a safe relative path: $stopCommand"
    }
    elseif ($stopCommand -and -not (Test-Path -LiteralPath (Join-Path $PluginRoot $stopCommand))) {
        $warnings += "entry.stopCommand does not exist: $stopCommand"
    }

    $installCommand = Get-OptionalString $Manifest.install "command"
    if (-not (Test-SafeRelativeCommand $installCommand)) {
        $warnings += "install.command is not a safe relative path: $installCommand"
    }
    elseif ($installCommand -and -not (Test-Path -LiteralPath (Join-Path $PluginRoot $installCommand))) {
        $warnings += "install.command does not exist: $installCommand"
    }

    return $warnings
}

function Get-OptionalString {
    param(
        [object]$Object,
        [string]$Name
    )

    if (-not $Object) {
        return ""
    }

    $property = $Object.PSObject.Properties[$Name]
    if (-not $property -or $null -eq $property.Value) {
        return ""
    }

    return [string]$property.Value
}

function Get-PropertyValue {
    param(
        [object]$Object,
        [string]$Name
    )

    if (-not $Object) {
        return $null
    }

    $property = $Object.PSObject.Properties[$Name]
    if (-not $property) {
        return $null
    }

    return $property.Value
}

function Test-ManifestShape {
    param([object]$Manifest)

    $errors = @()
    $warnings = @()

    foreach ($field in @("schemaVersion", "id", "name", "version")) {
        $value = Get-PropertyValue -Object $Manifest -Name $field
        if ($null -eq $value -or [string]::IsNullOrWhiteSpace([string]$value)) {
            $errors += "Manifest is missing required field: $field"
        }
    }

    $pluginId = Get-OptionalString $Manifest "id"
    if ($pluginId -and $pluginId -notmatch '^[a-z0-9][a-z0-9._-]*[a-z0-9]$') {
        $errors += "Manifest id should be lowercase package-style text: $pluginId"
    }

    $schemaVersion = Get-PropertyValue -Object $Manifest -Name "schemaVersion"
    if ($null -ne $schemaVersion -and [int]$schemaVersion -ne 1) {
        $warnings += "Manifest schemaVersion is not 1: $schemaVersion"
    }

    $compatibility = Get-PropertyValue -Object $Manifest -Name "compatibility"
    if (-not $compatibility) {
        $errors += "Manifest is missing required object: compatibility"
    }
    elseif ([string]::IsNullOrWhiteSpace((Get-OptionalString $compatibility "pluginApi"))) {
        $errors += "Manifest is missing required field: compatibility.pluginApi"
    }

    $entry = Get-PropertyValue -Object $Manifest -Name "entry"
    if (-not $entry) {
        $errors += "Manifest is missing required object: entry"
    }
    elseif ([string]::IsNullOrWhiteSpace((Get-OptionalString $entry "kind"))) {
        $errors += "Manifest is missing required field: entry.kind"
    }

    $capabilities = @(Get-PropertyValue -Object $Manifest -Name "capabilities")
    if ($capabilities.Count -eq 0 -or ($capabilities.Count -eq 1 -and $null -eq $capabilities[0])) {
        $errors += "Manifest must declare at least one capability."
        $capabilities = @()
    }

    $capabilityIds = @()
    foreach ($capability in $capabilities) {
        foreach ($field in @("id", "kind", "name", "version", "inputs", "outputs", "invoke")) {
            $value = Get-PropertyValue -Object $capability -Name $field
            if ($null -eq $value -or ([string]$value -eq "" -and $field -notin @("inputs", "outputs"))) {
                $errors += "Capability is missing required field '$field'."
            }
        }

        $capabilityId = Get-OptionalString $capability "id"
        if ($capabilityId) {
            $capabilityIds += $capabilityId
        }
    }

    $duplicateCapabilityIds = @($capabilityIds | Group-Object | Where-Object { $_.Count -gt 1 } | ForEach-Object { $_.Name })
    foreach ($duplicate in $duplicateCapabilityIds) {
        $errors += "Duplicate capability id: $duplicate"
    }

    $contributes = Get-PropertyValue -Object $Manifest -Name "contributes"
    $menus = @()
    if ($contributes) {
        $menus = @(Get-PropertyValue -Object $contributes -Name "menus")
        if ($menus.Count -eq 1 -and $null -eq $menus[0]) {
            $menus = @()
        }
    }

    $menuIds = @()
    foreach ($menu in $menus) {
        foreach ($field in @("id", "label", "capability", "contexts", "placements")) {
            $value = Get-PropertyValue -Object $menu -Name $field
            if ($null -eq $value -or ([string]$value -eq "" -and $field -notin @("contexts", "placements"))) {
                $errors += "Menu contribution is missing required field '$field'."
            }
        }

        $menuId = Get-OptionalString $menu "id"
        if ($menuId) {
            $menuIds += $menuId
        }

        $menuCapability = Get-OptionalString $menu "capability"
        if ($menuCapability -and $capabilityIds -notcontains $menuCapability) {
            $errors += "Menu '$menuId' references unknown capability: $menuCapability"
        }
    }

    $duplicateMenuIds = @($menuIds | Group-Object | Where-Object { $_.Count -gt 1 } | ForEach-Object { $_.Name })
    foreach ($duplicate in $duplicateMenuIds) {
        $errors += "Duplicate menu contribution id: $duplicate"
    }

    return [pscustomobject]@{
        Errors = @($errors | Sort-Object -Unique)
        Warnings = @($warnings | Sort-Object -Unique)
    }
}

function Should-ExcludePath {
    param(
        [string]$RelativePath,
        [bool]$IsDirectory
    )

    $parts = $RelativePath -split '[\\/]+' | Where-Object { $_ }
    foreach ($part in $parts) {
        $lower = $part.ToLowerInvariant()
        if ($lower -in @("__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache", "logs", "tmp", "temp")) {
            return $true
        }
        if ($lower -in @(".venv", "venv", "env", ".venv-cuda", ".venv-rocm", ".venv-cpu", ".venv-directml")) {
            return $true
        }
        if ($lower -in @("node_modules", ".git")) {
            return $true
        }
    }

    $fileName = [System.IO.Path]::GetFileName($RelativePath).ToLowerInvariant()
    if ($fileName.EndsWith(".pyc") -or $fileName.EndsWith(".pyo") -or $fileName.EndsWith(".log")) {
        return $true
    }

    if ($fileName -in @(".ds_store", "thumbs.db", ".env", ".local.env", "local.env")) {
        return $true
    }

    return $false
}

function Find-HardCodedPaths {
    param(
        [string]$PluginRoot,
        [System.IO.FileInfo[]]$Files
    )

    $warnings = @()
    $textExtensions = @(".json", ".bat", ".cmd", ".ps1", ".py")
    $absolutePathPattern = '(?i)([A-Z]:\\[^"''<>|\r\n]+)'

    foreach ($file in $Files) {
        if ($file.Extension.ToLowerInvariant() -notin $textExtensions) {
            continue
        }

        $relative = ConvertTo-RelativePath -Path $file.FullName -Base $PluginRoot
        $content = Get-Content -Raw -LiteralPath $file.FullName
        $matches = [regex]::Matches($content, $absolutePathPattern)
        foreach ($match in $matches) {
            $value = $match.Groups[1].Value.Trim()
            if ($value.StartsWith($PluginRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
                continue
            }
            $warnings += "Hard-coded absolute path in $relative`: $value"
        }
    }

    return $warnings | Sort-Object -Unique
}

function Find-NetworkPrivacyWarnings {
    param(
        [object]$Manifest,
        [string]$PluginRoot,
        [System.IO.FileInfo[]]$Files
    )

    $warnings = @()
    $textExtensions = @(".json", ".bat", ".cmd", ".ps1", ".py")
    $networkPattern = '(?i)\b(requests|urllib|socket|http://|https://)\b'

    $permissions = Get-PropertyValue -Object $Manifest -Name "permissions"
    $network = Get-PropertyValue -Object $permissions -Name "network"
    if ($network -is [bool]) {
        if ($network) {
            $warnings += "Manifest declares legacy permissions.network=true; review runtime network/privacy behavior."
        }
    } elseif ($network) {
        if ((Get-PropertyValue -Object $network -Name "runtime") -eq $true) {
            $warnings += "Manifest declares network.runtime=true; review privacy prompt and allowed domains."
        }
        if ((Get-PropertyValue -Object $network -Name "setupDownloads") -eq $true) {
            $allowedDomains = @(Get-PropertyValue -Object $network -Name "allowedDomains" | Where-Object { $_ -and "$_".Trim().Length -gt 0 })
            if ($allowedDomains.Count -eq 0) {
                $warnings += "Manifest declares network.setupDownloads=true without allowedDomains; review setup download disclosure."
            }
        }
        if ((Get-PropertyValue -Object $network -Name "uploadSelectedFiles") -eq $true) {
            $warnings += "Manifest declares network.uploadSelectedFiles=true; review selected-file upload disclosure."
        }
        if ((Get-PropertyValue -Object $network -Name "uploadOutputs") -eq $true) {
            $warnings += "Manifest declares network.uploadOutputs=true; review output upload disclosure."
        }
    }

    foreach ($file in $Files) {
        if ($file.Extension.ToLowerInvariant() -notin $textExtensions) {
            continue
        }

        if ($file.Name -match '^requirements.*\.txt$') {
            continue
        }

        $relative = ConvertTo-RelativePath -Path $file.FullName -Base $PluginRoot
        if ($relative -eq "picaipic.plugin.json") {
            continue
        }
        if ($relative -match '^backend\\main\.py$') {
            continue
        }
        $content = Get-Content -Raw -LiteralPath $file.FullName
        $contentForScan = $content.Replace("https://git-lfs.github.com/spec/v1", "")
        if ($contentForScan -match $networkPattern) {
            $warnings += "Potential network-related code or URL found in $relative"
        }
    }

    return $warnings | Sort-Object -Unique
}

function Test-WindowsBatchFiles {
    param(
        [string]$PluginRoot,
        [System.IO.FileInfo[]]$Files
    )

    $errors = @()
    foreach ($file in $Files) {
        $extension = $file.Extension.ToLowerInvariant()
        if ($extension -notin @(".bat", ".cmd")) {
            continue
        }

        $relative = ConvertTo-RelativePath -Path $file.FullName -Base $PluginRoot
        $bytes = [System.IO.File]::ReadAllBytes($file.FullName)
        if ($bytes -contains 0) {
            $errors += "$relative contains NUL bytes; rewrite it as plain text before packaging."
        }

        $content = [System.Text.Encoding]::Default.GetString($bytes)
        if ($content -match "(?<!`r)`n") {
            $errors += "$relative uses LF line endings; Windows .bat/.cmd files must use CRLF."
        }

        if ($content.Contains("%%A:~0,1%")) {
            $errors += "$relative uses the old .local.env parser that fails under cmd.exe; use for /f `"usebackq eol=# tokens=1,* delims==`" instead."
        }
    }

    return $errors | Sort-Object -Unique
}

function New-PackageManifest {
    param(
        [string]$PluginRoot,
        [string]$PluginId,
        [string]$Version,
        [System.IO.FileInfo[]]$Files,
        [string[]]$Warnings
    )

    $fileRows = foreach ($file in ($Files | Sort-Object FullName)) {
        $hash = Get-FileHash -Algorithm SHA256 -LiteralPath $file.FullName
        [ordered]@{
            path = (ConvertTo-RelativePath -Path $file.FullName -Base $PluginRoot).Replace("\", "/")
            size = $file.Length
            sha256 = $hash.Hash
        }
    }

    return [ordered]@{
        schemaVersion = 1
        packageKind = "picaipic-plugin-package"
        pluginId = $PluginId
        version = $Version
        createdAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        files = @($fileRows)
        warnings = @($Warnings)
    }
}

function Package-Plugin {
    param(
        [string]$PluginRoot,
        [string]$OutputRoot,
        [string]$StagingRoot,
        [string]$SignKeyFile
    )

    $pluginRootResolved = (Resolve-Path $PluginRoot).Path
    $manifestPath = Join-Path $pluginRootResolved "picaipic.plugin.json"
    if (-not (Test-Path -LiteralPath $manifestPath)) {
        throw "Plugin root does not contain picaipic.plugin.json: $pluginRootResolved"
    }

    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
    $manifestResult = Test-ManifestShape -Manifest $manifest
    foreach ($errorItem in $manifestResult.Errors) {
        Write-Warn $errorItem
    }
    foreach ($warningItem in $manifestResult.Warnings) {
        Write-Warn $warningItem
    }
    if ($manifestResult.Errors.Count -gt 0) {
        throw "Manifest validation failed for $manifestPath"
    }

    $pluginId = [string]$manifest.id
    $version = [string]$manifest.version
    if (-not $pluginId -or -not $version) {
        throw "Manifest must contain id and version: $manifestPath"
    }

    Write-Step "Packaging $pluginId $version"

    $allFiles = @(Get-ChildItem -LiteralPath $pluginRootResolved -Recurse -File -Force)
    $includedFiles = @(
        $allFiles | Where-Object {
            $relative = ConvertTo-RelativePath -Path $_.FullName -Base $pluginRootResolved
            -not (Should-ExcludePath -RelativePath $relative -IsDirectory:$false)
        }
    )

    if (-not ($includedFiles | Where-Object { $_.Name -eq "README.md" })) {
        Write-Warn "README.md is missing from $pluginId."
    }

    $batchErrors = @(Test-WindowsBatchFiles -PluginRoot $pluginRootResolved -Files $includedFiles)
    foreach ($errorItem in $batchErrors) {
        Write-Warn $errorItem
    }
    if ($batchErrors.Count -gt 0) {
        throw "Windows batch validation failed for $pluginId."
    }

    $warnings = @()
    $warnings += $manifestResult.Warnings
    $warnings += Get-ManifestCommandWarnings -Manifest $manifest -PluginRoot $pluginRootResolved
    $warnings += Find-HardCodedPaths -PluginRoot $pluginRootResolved -Files $includedFiles
    $warnings += Find-NetworkPrivacyWarnings -Manifest $manifest -PluginRoot $pluginRootResolved -Files $includedFiles
    $warnings = @($warnings | Sort-Object -Unique)

    foreach ($warning in $warnings) {
        Write-Warn $warning
    }

    if ($FailOnWarnings -and $warnings.Count -gt 0) {
        throw "$pluginId has packaging warnings and -FailOnWarnings was set."
    }

    $safeName = "$pluginId-$version"
    $packageStagingRoot = Join-Path $StagingRoot $safeName
    $packageContentRoot = Join-Path $packageStagingRoot $pluginId
    Remove-DirectorySafe -Path $packageStagingRoot -AllowedRoot $StagingRoot
    New-Item -ItemType Directory -Path $packageContentRoot -Force | Out-Null

    foreach ($file in $includedFiles) {
        $relative = ConvertTo-RelativePath -Path $file.FullName -Base $pluginRootResolved
        $target = Join-Path $packageContentRoot $relative
        $targetDir = Split-Path -Parent $target
        if (-not (Test-Path -LiteralPath $targetDir)) {
            New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
        }
        Copy-Item -LiteralPath $file.FullName -Destination $target -Force
    }

    $packageManifest = New-PackageManifest `
        -PluginRoot $pluginRootResolved `
        -PluginId $pluginId `
        -Version $version `
        -Files $includedFiles `
        -Warnings $warnings
    $packageManifestPath = Join-Path $packageContentRoot "picaipic.package.json"
    $packageManifest | ConvertTo-Json -Depth 8 | Set-Content -Path $packageManifestPath -Encoding UTF8

    # Sign the package manifest if a private key file is provided.
    if ($SignKeyFile) {
        $signKeyPath = (Resolve-Path $SignKeyFile -ErrorAction SilentlyContinue)
        if (-not $signKeyPath) {
            throw "Sign key file not found: $SignKeyFile"
        }
        $privateKeyB64 = (Get-Content -LiteralPath $signKeyPath.Path -Raw).Trim()
        $signScript = Join-Path $RootDir "scripts\sign_plugin.py"
        Write-Step "Signing package manifest for $pluginId"
        & python $signScript sign $packageManifestPath $privateKeyB64
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to sign package manifest for plugin '$pluginId'."
        }
        Write-Ok "Package manifest signed."
    }

    if (-not (Test-Path -LiteralPath $OutputRoot)) {
        New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
    }

    $zipPath = Join-Path $OutputRoot "$safeName.zip"
    if (Test-Path -LiteralPath $zipPath) {
        Remove-Item -LiteralPath $zipPath -Force
    }

    Compress-Archive -Path (Join-Path $packageStagingRoot $pluginId) -DestinationPath $zipPath -CompressionLevel Optimal
    $zipItem = Get-Item -LiteralPath $zipPath
    $zipHash = Get-FileHash -Algorithm SHA256 -LiteralPath $zipPath

    Write-Ok "Created $zipPath"
    Write-Host "    Size: $([math]::Round($zipItem.Length / 1MB, 2)) MB"
    Write-Host "    SHA256: $($zipHash.Hash)"
    Write-Host "    Included files: $($includedFiles.Count + 1)"

    if (-not $KeepStaging) {
        Remove-DirectorySafe -Path $packageStagingRoot -AllowedRoot $StagingRoot
    }

    return [pscustomobject]@{
        PluginId = $pluginId
        Version = $version
        Zip = $zipPath
        SizeMB = "{0:N2}" -f ($zipItem.Length / 1MB)
        SHA256 = $zipHash.Hash
        Warnings = $warnings.Count
    }
}

$RootDir = Resolve-RepoRoot
if (-not $OutputDir) {
    $OutputDir = Join-Path $RootDir "dist\plugins"
}
$OutputRoot = [System.IO.Path]::GetFullPath($OutputDir)
$StagingRoot = Join-Path $OutputRoot ".staging"
New-Item -ItemType Directory -Path $StagingRoot -Force | Out-Null

if ($All) {
    $pluginRoots = @(
        Get-ChildItem -LiteralPath (Join-Path $RootDir "plugins") -Directory |
            Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName "picaipic.plugin.json") } |
            ForEach-Object { $_.FullName }
    )
}
else {
    if (-not $PluginPath) {
        throw "Pass a plugin path, or use -All."
    }
    $pluginRoots = @($PluginPath)
}

if ($pluginRoots.Count -eq 0) {
    throw "No plugin roots found."
}

$results = @()
try {
    foreach ($root in $pluginRoots) {
        $results += Package-Plugin -PluginRoot $root -OutputRoot $OutputRoot -StagingRoot $StagingRoot -SignKeyFile $SignKeyFile
    }
}
finally {
    if (-not $KeepStaging -and (Test-Path -LiteralPath $StagingRoot)) {
        $remaining = @(Get-ChildItem -LiteralPath $StagingRoot -Force)
        if ($remaining.Count -eq 0) {
            Remove-Item -LiteralPath $StagingRoot -Force
        }
    }
}

Write-Step "Package summary"
$results | Format-Table -AutoSize
