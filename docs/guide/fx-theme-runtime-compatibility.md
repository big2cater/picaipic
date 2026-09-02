# Dynamic Theme Runtime Compatibility

Updated: 2026-07-30

PicAiPic ships two dynamic themes: Black hole and Cyberpunk. Their ambient
backgrounds and photo-area idle effects use separate render paths, so a working
background does not by itself prove that thumbnail distortion is active.

## Activation Contract

The photo-area effect runs only when all of these conditions are true:

- the matching dynamic theme is active;
- the main Home window is natively maximized;
- there has been no mouse, keyboard, wheel, scroll, or touch input for 6 seconds;
- Windows/WebView does not request reduced motion;
- the document is visible;
- no blocking input stack is open; and
- no library switch is in progress.

Home queries its own Tauri window state at startup and after resize events.
TitleBar also refreshes native maximize state after maximize/unmaximize completes.
Settings and auxiliary windows never write the main-window maximize gate.

`dynamicThemeIntensity` supports `0`, `0.5`, `1`, and `1.5`. Startup migration
sets a missing or invalid legacy value to `1`; an explicit `0` remains disabled.

## Black Hole

- Ambient path: `BlackHoleBackground` uses WebGL with a Canvas2D fallback.
- Primary photo path: `PhotoVortexLayer` captures visible thumbnails into one
  texture and applies the continuous UV vortex.
- Cross-GPU protection: capture and render buffers are clamped to
  `MAX_TEXTURE_SIZE` and `MAX_VIEWPORT_DIMS`.
- Failure path: unavailable WebGL, an empty/blocked thumbnail capture, texture
  upload failure, or context loss leaves the live grid visible and activates
  the existing `useGravityWarp` per-card CSS spiral.

## Cyberpunk

- Ambient path: `CyberpunkBackground` renders the Home-only city backdrop;
  reduced motion keeps the backdrop static and disables the photo glitch.
- Primary photo path: `PhotoGlitchLayer` captures visible thumbnails and runs
  the WebGL RGB-separation, scanline, grain, and displacement shader.
- Cross-GPU protection: capture and render buffers use the same GPU-limit
  clamping as the black-hole photo layer.
- Failure path: WebGL/capture/upload/context failure keeps live cards visible
  and applies the CSS translation, contrast/color, and cyan/magenta edge glitch.

## Windows Package Verification

Use PowerShell 7 for local Windows commands. Build a clean package with:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File .\scripts\package_windows.ps1 -Clean
```

The generated Tauri override runs `pnpm --dir src-vite build` from the repository
root. Frontend assets are embedded in `PicAiPic.exe`, so the install directory
does not select a different theme bundle.

For same-version `1.1.0` testing, prefer the NSIS setup executable. Windows
Installer may reuse a cached MSI for an already-installed identical version;
uninstall the old MSI or bump the version before validating a replacement MSI.

## Manual Matrix

Test both themes on a normal-DPI desktop and a high-DPI/integrated-GPU laptop:

1. Select the theme and set intensity to Standard.
2. Maximize the main window and leave it idle for at least 6 seconds.
3. Confirm the ambient background and photo-area effect both run.
4. Move the pointer and confirm the live grid returns immediately.
5. Repeat with Windows Animation effects disabled and confirm dynamic photo
   effects remain off by accessibility design.
6. Repeat at intensity Off and confirm the ambient theme remains while the
   photo effect stays disabled.

