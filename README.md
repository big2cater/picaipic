<div align="center">
  <img src="docs/public/icon.png" alt="PicAiPic Logo" width="120" style="border-radius: 20px">
  <h1>PicAiPic - Private Local Photo Manager</h1>
  <h3>Open-source desktop photo manager for Windows and Linux.</h3>
  <p>
    <a href="https://github.com/big2cater/picaipic/releases"><img src="https://img.shields.io/github/v/release/big2cater/picaipic" alt="GitHub release"></a>
    <a href="https://github.com/big2cater/picaipic/releases"><img src="https://img.shields.io/github/downloads/big2cater/picaipic/total" alt="GitHub all releases"></a>
    <a href="https://github.com/big2cater/picaipic/stargazers"><img src="https://img.shields.io/github/stars/big2cater/picaipic" alt="GitHub stars"></a>
  </p>
</div>


English | [简体中文](i18n/README.zh-CN.md)

PicAiPic is an open-source, local-first photo manager for browsing family albums, finding old photos quickly, and managing large personal media libraries offline.
It is a privacy-focused alternative to cloud photo services: no forced upload, local AI search, folder-first workflow, and free to use.

- Website: [https://big2cater.github.io/picaipic/](https://big2cater.github.io/picaipic/)
- Demo: [https://youtu.be/RbKqNKhbVUs](https://youtu.be/RbKqNKhbVUs)
- Privacy: [PRIVACY.md](PRIVACY.md)

## Download PicAiPic

Open the [latest release page](https://github.com/big2cater/picaipic/releases/latest), then download the file that matches your system:

| Platform | Package | Note |
| :-- | :-- | :-- |
| **Windows 10/11 (x64 / ARM64)** | `_x64_en-US.msi` / `_arm64_en-US.msi` | Unsigned — if SmartScreen blocks the download, click **Keep anyway** |
| **Linux (amd64 / arm64)** | `_amd64.deb` / `_arm64.deb` | For Debian-based distros (Ubuntu, Debian, Linux Mint, etc.) |

## Screenshots

<p align="center">
  <img src="docs/public/screenshots/lap-home-0.1.10_1.png" alt="PicAiPic local photo library manager screenshot" width="900">
  </p>

  <p align="center">
  <img src="docs/public/screenshots/lap-home-0.1.10_2.png" alt="PicAiPic local AI photo search screenshot" width="900">
</p>

## Why PicAiPic

- **Local-first by design**: your photos stay on your own disk, with no required cloud account or upload.
- **No library lock-in**: work directly with your existing folders instead of importing everything into a closed database.
- **Private AI tools**: search, similarity, smart tags, and face features run locally on your machine.
- **Built for large collections**: optimized for smooth browsing and organization across libraries with 100k+ files.
- **Open source and free**: no subscription, no forced ecosystem, and code you can inspect.

## Features

- **Fast library browsing** with timeline, folder, location, camera, lens, tag, favorite, rating, and face filters.
- **Local AI search** for text prompts, visual similarity, smart tags, face clustering, and optional multilingual search in 50+ languages.
- **Folder-first workflow** with multiple libraries, drag-and-drop import, copy-paste import, filesystem sync, and safe move/copy/delete operations.
- **Cleanup tools** to find duplicates and batch move unwanted files to trash.
- **Built-in editing** for crop, rotate, flip, resize, and basic image adjustments.
- **Broad format support** for 60+ photo, RAW, and video formats.

## Uninstall PicAiPic

PicAiPic works directly with your existing photo folders. Uninstalling PicAiPic, or deleting its database and cache files, does **not** delete your original photos.

The standard uninstall steps remove the application. To remove PicAiPic completely, quit PicAiPic first, uninstall the application, then delete its local database, thumbnail cache, and configuration files using the cleanup command for your platform.

### Windows

Open **Settings > Apps > Installed apps**, find **PicAiPic**, and select **Uninstall**.

Then open PowerShell and remove all PicAiPic database, cache, and configuration files:

```powershell
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "$env:LOCALAPPDATA\com.big2cater.picaipic"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "$env:APPDATA\com.big2cater.picaipic"
```

### Linux

For Debian-based distributions, uninstall the package:

```bash
sudo apt remove picaipic
```

Then remove all PicAiPic database, cache, and configuration files:

```bash
rm -rf "$HOME/.local/share/com.big2cater.picaipic" \
       "$HOME/.cache/com.big2cater.picaipic" \
       "$HOME/.config/com.big2cater.picaipic"
```

If you selected a custom database storage directory in PicAiPic settings, delete that directory separately after confirming that it contains only PicAiPic database files.

## Current Development Focus

The current frontend/backend work is centered on the AI plugin host pipeline and packaged plugin experience. The most important next steps are:

- validate the plugin lifecycle in the release executable (Start / Restart / Smoke)
- runtime-conflict detection is implemented: the host blocks capability calls when a shared runtime's installed packages drift from the plugin's pinned requirements, and advises switching to a plugin-private runtime; one-click auto-switch is still future work
- plugin uninstall now offers a choice between "code only" and "code + data & runtimes" (shared runtimes are always kept)
- security hardening A+B+C are landed: startup bearer-token auth (A), Ed25519 package signing with a user-managed trust store (B), and v1 plugin confinement via input-file staging (C); the experimental Windows deny-ACL write confinement path is now explicit opt-in because it changes real user-directory ACLs; network blocking and Linux sandboxing are future work
- add model import and external model directory binding support

## Build from Source

Requirements: Node.js 20+, pnpm, Rust stable.

```bash
# Linux system deps
# sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev \
#   patchelf nasm clang pkg-config autoconf automake libtool cmake

# Clone and build
git clone --recursive https://github.com/big2cater/picaipic.git
cd picaipic
git submodule update --init --recursive
cargo install tauri-cli --version "^2.0.0" --locked
./scripts/download_models.sh            # Windows: .\scripts\download_models.ps1
./scripts/download_ffmpeg_sidecar.sh    # Windows: .\scripts\download_ffmpeg_sidecar.ps1
cd src-vite && pnpm install && cd ..
cargo tauri dev
```

## Supported Formats

PicAiPic supports 60+ photo, RAW, and video formats.

| Type | Formats |
| :--- | :--- |
| Images | JPG/JPEG, PNG, GIF, BMP, TIFF, WebP, HEIC/HEIF/HIF, AVIF, JXL, PSD, EXR, HDR/RGBE, TGA, JPEG 2000 (JP2/J2K/J2C/JPC/JPF/JPX), DDS, DPX, QOI |
| RAW photos | CR2, CR3, CRW, NEF, NRW, ARW, SRF, SR2, RAF, RW2, ORF, PEF, DNG, SRW, RWL, MRW, 3FR, MOS, DCR, KDC, ERF, MEF, RAW, MDC |
| Videos | MP4, MOV, M4V, MKV, AVI, FLV, TS/M2TS, WMV, WebM, 3GP/3G2, F4V, VOB, MPG/MPEG, ASF, DIVX and more. H.264 playback is supported on all platforms, with automatic compatibility processing when native playback is unavailable. |

### Linux Video Playback Notes

On Ubuntu/Debian/Linux Mint, install these packages for better video playback support:

```bash
sudo apt install gstreamer1.0-libav gstreamer1.0-plugins-good
```

## Architecture

- Core: Tauri + Rust
- Frontend: Vue + Vite + Tailwind CSS
- Data: SQLite

### Key Libraries

| Library | Purpose |
| :-- | :-- |
| [LibRaw](https://github.com/LibRaw/LibRaw) | RAW image decoding and thumbnail extraction |
| [libheif](https://github.com/strukturag/libheif) | HEIC/HEIF/HIF image decoding and preview generation |
| [libjpeg-turbo](https://libjpeg-turbo.org/) | Fast JPEG decoding and thumbnail generation |
| [FFmpeg](https://ffmpeg.org/) | Video processing and thumbnail generation |
| [Video.js](https://videojs.com/) | Cross-platform video playback UI |
| [ONNX Runtime](https://onnxruntime.ai/) | Local AI model inference engine |
| [CLIP](https://github.com/openai/CLIP) | Image-text similarity search |
| [InsightFace](https://github.com/deepinsight/insightface) | Face detection and recognition |
| [Leaflet](https://leafletjs.com/) | Interactive map for geotagged photos |
| [daisyUI](https://daisyui.com/) | UI component library |

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
