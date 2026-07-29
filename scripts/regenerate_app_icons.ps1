# Regenerate PicAiPic Windows/app icons from the canonical mark.
# Source of truth: repo-root favicon1.ico (neural-cat / PicAiPic brand).
# Frame default logo (resources/branding/default-frame-logo.png) is SEPARATE — do not use it here.
#
# Usage (from repo root):
#   powershell -ExecutionPolicy Bypass -File .\scripts\regenerate_app_icons.ps1
# Then rebuild installers so the new ICO is linked into the EXE:
#   cargo clean -p PicAiPic
#   .\scripts\package_windows.ps1   # or cargo tauri build

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Copy-FileIfChanged {
  param(
    [Parameter(Mandatory = $true)][string]$Source,
    [Parameter(Mandatory = $true)][string]$Destination
  )

  if (Test-Path -LiteralPath $Destination) {
    $sourceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Source).Hash
    $destinationHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Destination).Hash
    if ($sourceHash -eq $destinationHash) {
      return $false
    }
  }

  $destinationDir = Split-Path -Parent $Destination
  New-Item -ItemType Directory -Force -Path $destinationDir | Out-Null
  try {
    Copy-Item -LiteralPath $Source -Destination $Destination -Force
  }
  catch [System.IO.IOException] {
    throw "Cannot update icon '$Destination'. Close PicAiPic and any preview window using this file, then retry. $($_.Exception.Message)"
  }
  return $true
}

$Favicon = Join-Path $Root "favicon1.ico"
if (-not (Test-Path $Favicon)) {
  throw "Missing canonical icon: $Favicon"
}

$MasterPng = Join-Path $Root "src-tauri\app-icon.png"
$IconsDir = Join-Path $Root "src-tauri\icons"
$BrandingDir = Join-Path $Root "src-tauri\resources\branding"
$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("picaipic-icons-" + [guid]::NewGuid().ToString("N"))
$TempMasterPng = Join-Path $TempRoot "app-icon.png"
$TempIconsDir = Join-Path $TempRoot "icons"
New-Item -ItemType Directory -Force -Path $TempRoot | Out-Null

try {
  # Extract largest PNG from favicon1.ico via Python+Pillow.
  python -c @"
from pathlib import Path
from PIL import Image
from io import BytesIO
import struct
data = Path(r'$Favicon').read_bytes()
count = struct.unpack_from('<H', data, 4)[0]
off = 6
best = None
for i in range(count):
    w,h,_,_,_,_, size, offset = struct.unpack_from('<BBBBHHII', data, off)
    off += 16
    chunk = data[offset:offset+size]
    if chunk[:8] == b'\x89PNG\r\n\x1a\n':
        im = Image.open(BytesIO(chunk)).convert('RGBA')
        if best is None or im.size[0] > best.size[0]:
            best = im
assert best is not None
master = best.resize((1024, 1024), Image.Resampling.LANCZOS) if best.size[0] < 1024 else best
master.save(r'$TempMasterPng', 'PNG')
print('master', master.size)
"@
  if ($LASTEXITCODE -ne 0) {
    throw "Failed to extract the master PNG from favicon1.ico (exit code $LASTEXITCODE)."
  }

  # Generate away from the live app. Windows WebView may memory-map PNG files,
  # which prevents tools from opening the repository copies for replacement.
  pnpm --dir (Join-Path $Root "src-vite") exec tauri icon $TempMasterPng -o $TempIconsDir
  if ($LASTEXITCODE -ne 0) {
    throw "Failed to generate the Tauri icon set (exit code $LASTEXITCODE)."
  }

  $updated = 0
  if (Copy-FileIfChanged $Favicon (Join-Path $BrandingDir "app-icon-source.ico")) { $updated++ }
  if (Copy-FileIfChanged $TempMasterPng $MasterPng) { $updated++ }

  # PicAiPic supports Windows and Linux only. Root Tauri icon files cover
  # those platforms; Android, iOS, and macOS outputs remain temporary.
  $generatedFiles = Get-ChildItem -LiteralPath $TempIconsDir -File | Where-Object {
    $_.Name -ne "icon.icns"
  }

  $generatedFiles | ForEach-Object {
    $relativePath = $_.FullName.Substring($TempIconsDir.Length).TrimStart('\', '/')
    if (Copy-FileIfChanged $_.FullName (Join-Path $IconsDir $relativePath)) { $updated++ }
  }

  # Frontend chrome + docs site use the generated high-resolution PNG.
  $generatedIconPng = Join-Path $TempIconsDir "icon.png"
  if (Copy-FileIfChanged $generatedIconPng (Join-Path $Root "src-vite\src\assets\images\icon.png")) { $updated++ }
  if (Copy-FileIfChanged $generatedIconPng (Join-Path $Root "docs\public\icon.png")) { $updated++ }

  Write-Host "Synchronized $updated changed icon file(s); identical files were left untouched."
}
finally {
  Remove-Item -LiteralPath $TempRoot -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host @"

Icons regenerated from favicon1.ico into src-tauri/icons/.

IMPORTANT — icons are baked into the EXE at link time:
  cargo clean -p PicAiPic
  cargo tauri build
  # or package_windows.ps1

Without clean/rebuild, Windows may keep the previous embedded icon.
Do NOT copy logo-pic.png / default-frame-logo.png into icons/.
"@
