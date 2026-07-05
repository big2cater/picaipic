# Release Build - 2026-06-30

## Command

Built from:

```powershell
cd D:\ailab\PicAiPic\src-tauri
tauri build
```

Before this build, stale runtime/build artifacts were removed:

- `D:\ailab\PicAiPic\src-tauri\target\release\Lap.exe`
- `D:\ailab\PicAiPic\src-tauri\target\release\Lap.pdb`
- `D:\ailab\PicAiPic\src-tauri\target\release\bundle`
- `C:\Users\a7925\AppData\Local\com.julyx10.lap\EBWebView`

## Outputs

| Artifact | Size | Last write time | SHA256 |
| --- | ---: | --- | --- |
| `D:\ailab\PicAiPic\src-tauri\target\release\Lap.exe` | 43.48 MB | 2026-06-30 18:46:50 | `F818A1A30651EF4607813C32E47E00CBBD19F57D9B0FABAF6FD336AA28E9B776` |
| `D:\ailab\PicAiPic\src-tauri\target\release\bundle\nsis\Lap_0.2.4_x64-setup.exe` | 183.60 MB | 2026-06-30 18:46:49 | `ED4F660440F713B84404FFC9A9A005A378FC3EC703C013775AD2FF2F37D90033` |
| `D:\ailab\PicAiPic\src-tauri\target\release\bundle\msi\Lap_0.2.4_x64_en-US.msi` | 216.75 MB | 2026-06-30 18:44:53 | `E11FF1D8C4C5BE2C1BB3CFFC356DBD43BB71C3E8B1CBCCEB43B399CCE4CC590B` |

## Verification

- Release frontend build completed.
- Rust release build completed.
- NSIS and MSI installers were generated.
- The build includes the AI plugin stop/running-state fix:
  - host status distinguishes `managed` runtime state from stale localhost reachability
  - Settings UI only shows Running for host-managed reachable runtimes
  - NAFNet/SA-LUT stop scripts include stale backend cleanup helpers

## Signing note

The executable and installers were generated successfully. The final `tauri build` command exited non-zero only because updater signing found a public key but no private key:

```text
A public key has been found, but no private key. Make sure to set TAURI_SIGNING_PRIVATE_KEY environment variable.
```

This affects updater signature artifact generation, not the generated local `Lap.exe`, NSIS installer, or MSI installer files listed above.

## Runtime data note

Release builds use the formal app data directory:

- `C:\Users\a7925\AppData\Local\com.julyx10.lap`

Debug builds use:

- `C:\Users\a7925\AppData\Local\com.julyx10.lap.debug`

The formal release registry currently contains both plugin paths:

- `D:\ailab\PicAiPic\plugins\picai-nafnet-restore`
- `D:\ailab\PicAiPic\plugins\picai-salut-color`

If a release run still appears to show an old plugin UI, verify that the launched process path is the new artifact:

- `D:\ailab\PicAiPic\src-tauri\target\release\Lap.exe`

Also clear the formal WebView cache if needed:

- `C:\Users\a7925\AppData\Local\com.julyx10.lap\EBWebView`

This cache was cleared before the 18:46 build.
