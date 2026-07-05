# AI Plugin Stop/Running State Fix - 2026-06-30

## Symptom

In release UI, clicking Stop for `picai-nafnet-restore` could still leave the plugin card showing `Running`.

## Root cause

This is primarily a host/runtime-state contract issue, with a plugin stop-script hardening gap.

The host used two different concepts as one UI state:

- a runtime process currently started and tracked by PicAiPic
- any localhost service reachable at the manifest/default plugin port

If an older/dev backend was still listening on NAFNet's default port `8012`, status refresh could report the plugin as reachable and the UI would show `Running`, even after the host stopped its tracked child process.

On this machine, port `8012` was owned by a stale `python.exe` PID with empty `CommandLine`/`ExecutablePath`. Even `taskkill /PID 21596 /T /F` returned `Access is denied`, so it could not be cleaned up from normal user context.

## Host fix

Changed `AiPluginStatus` to include `managed`.

- `reachable=true, managed=true`: current PicAiPic process owns/tracks this runtime; UI may show Running.
- `reachable=true, managed=false`: an external/stale service is reachable; UI must not show this as the current plugin running. Settings should show it as a stale/external service, and plugin menus must not treat it as an available managed plugin.
- Stop no longer falls back to probing the manifest default port after removing the tracked runtime.
- Start no longer short-circuits just because an unmanaged default-port backend is reachable; it can allocate a fresh port and start a managed runtime.

2026-07-02 follow-up:

- The plugin menu store now only treats `reachable && managed` plugins as running.
- Settings status badges now distinguish normal managed `Running` from stale/external reachable services.
- Shutdown cleanup was strengthened to scan discovered plugin manifests and call their stop paths, not only stop runtimes currently present in the in-memory process map.
- Port cleanup now uses `netstat -ano` plus `taskkill` instead of `Get-NetTCPConnection`, avoiding a slow shutdown path on Windows.

Changed files:

- `D:\ailab\PicAiPic\src-tauri\src\t_plugin.rs`
- `D:\ailab\PicAiPic\src-vite\src\views\Settings.vue`

## Plugin stop hardening

Added plugin-owned stop helpers that try to clean stale Python backends by script path and by plugin port:

- `D:\ailab\PicAiPic\plugins\picai-nafnet-restore\backend\stop_plugin.ps1`
- `D:\ailab\PicAiPic\plugins\picai-salut-color\backend\stop_plugin.ps1`

Updated:

- `D:\ailab\PicAiPic\plugins\picai-nafnet-restore\stop.bat`
- `D:\ailab\PicAiPic\plugins\picai-salut-color\stop.bat`

If Windows refuses to kill an old process with `Access is denied`, the host fix still prevents UI from reporting that stale process as the currently running managed plugin.

## Verification

Passed after changes:

```powershell
cargo check --manifest-path D:\ailab\PicAiPic\src-tauri\Cargo.toml
cd D:\ailab\PicAiPic\src-vite
npm run build
```

2026-07-02 follow-up verification:

```powershell
pnpm --dir D:\ailab\PicAiPic\src-vite build
```

## Current user-facing answer

This should be treated as a host interface/state problem exposed by a stale plugin backend. The plugin stop scripts are now more defensive, but the important v1 contract fix is that host UI distinguishes managed runtime from unmanaged reachable localhost services.
