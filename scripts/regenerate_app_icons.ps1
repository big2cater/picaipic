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

$Favicon = Join-Path $Root "favicon1.ico"
if (-not (Test-Path $Favicon)) {
  throw "Missing canonical icon: $Favicon"
}

$MasterPng = Join-Path $Root "src-tauri\app-icon.png"
$IconsDir = Join-Path $Root "src-tauri\icons"
$BrandingDir = Join-Path $Root "src-tauri\resources\branding"
New-Item -ItemType Directory -Force -Path $BrandingDir | Out-Null
Copy-Item -Force $Favicon (Join-Path $BrandingDir "app-icon-source.ico")

# Extract largest PNG from favicon1.ico via Python+Pillow
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
master.save(r'$MasterPng', 'PNG')
print('master', master.size)
"@

# Official Tauri icon set (icon.ico / icon.png / 32 / 128 / icns / Square* / …)
pnpm --dir (Join-Path $Root "src-vite") exec tauri icon $MasterPng -o $IconsDir

# Frontend chrome + docs site
Copy-Item -Force (Join-Path $IconsDir "icon.png") (Join-Path $Root "src-vite\src\assets\images\icon.png")
Copy-Item -Force (Join-Path $IconsDir "icon.png") (Join-Path $Root "docs\public\icon.png")

Write-Host @"

Icons regenerated from favicon1.ico into src-tauri/icons/.

IMPORTANT — icons are baked into the EXE at link time:
  cargo clean -p PicAiPic
  cargo tauri build
  # or package_windows.ps1

Without clean/rebuild, Windows may keep the previous embedded icon.
Do NOT copy logo-pic.png / default-frame-logo.png into icons/.
"@
