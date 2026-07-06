# Getting Started

## Installation

### Linux (Ubuntu/Debian/Linux Mint, amd64 / arm64)

1. Download the latest `_amd64.deb` or `_arm64.deb` package from the [Releases page](https://github.com/big2cater/picaipic/releases).
2. Install it with your package manager or run `sudo apt install ./picaipic_<version>_amd64.deb` or `sudo apt install ./picaipic_<version>_arm64.deb`.
3. Launch **PicAiPic** from your applications menu.

For better video playback support on Ubuntu/Debian/Linux Mint, install:

```bash
sudo apt install gstreamer1.0-libav gstreamer1.0-plugins-good
```

### Windows 10/11 (x64 / ARM64)

1. Download the latest `_x64_en-US.msi` or `_arm64_en-US.msi` installer from the [Releases page](https://github.com/big2cater/picaipic/releases/latest).
2. Run the installer and complete the setup wizard.
3. Launch **PicAiPic** from the Start menu or desktop shortcut.

PicAiPic's Windows installer is currently unsigned. If Microsoft SmartScreen blocks the download or installer, choose **Keep anyway** or **More info** > **Run anyway**.

## First Run

When you open PicAiPic for the first time:

1.  **Grant Permissions**: PicAiPic needs access to your folders to display photos.
2.  **Add a Library**: Point PicAiPic to a folder containing your photos.
3.  **Let it Index**: PicAiPic will scan your files, generate thumbnails, and build local search data. This happens on your device.

## Upgrading from v0.1.x

You can install PicAiPic v0.2.x directly over a v0.1.x installation. The local database is migrated automatically on first launch.
