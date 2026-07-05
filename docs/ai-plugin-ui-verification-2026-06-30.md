# AI Plugin Host UI Verification - 2026-06-30

## Scope

This note records the host UI verification pass after freezing the v1 plugin contract baseline.

## Dev launch

Command used:

```powershell
cd D:\ailab\PicAiPic\src-tauri
tauri dev --no-watch
```

Observed startup log:

- DB version check completed.
- AI Models loaded successfully.
- AI Engine started successfully.
- `Lap.exe` is running and responding.

## Main window

The PicAiPic/Lap main window opened successfully and displayed the local album grid.

Screenshot artifact:

- `D:\ailab\PicAiPic\tmp_lap_only.png`

## Settings window

The Settings window opened successfully from the host UI.

Screenshot artifact:

- `D:\ailab\PicAiPic\tmp_settings.png`

Automated coordinate clicking did not reliably switch from the General tab to the Plugin tab in this desktop session. The likely reason is Windows foreground/focus/transparent-window behavior during automation, not a plugin host contract failure.

## Backend/UI data confirmed

The debug registry contains both real plugin paths:

- `D:\ailab\PicAiPic\plugins\picai-nafnet-restore`
- `D:\ailab\PicAiPic\plugins\picai-salut-color`

The Settings plugin tab implementation renders from the same `list_ai_plugins` Tauri command and includes controls for:

- plugin directories
- installed plugin cards
- Start
- Stop
- Restart
- Refresh status
- Diagnostics
- Logs
- runtime profile selection
- Probe Runtime
- Setup
- Run Setup
- Verify
- Smoke
- test invoke

Source reference:

- `D:\ailab\PicAiPic\src-vite\src\views\Settings.vue`

## Verification status

| Item | Status | Notes |
| --- | --- | --- |
| Dev launch | Pass | `Lap.exe` started and responded |
| Main window display | Pass | Album grid visible |
| Settings window open | Pass | Settings modal/window visible |
| Plugin registry contains SA-LUT | Pass | Registry path present |
| Plugin registry contains NAFNet | Pass | Registry path present |
| Settings plugin tab visual check | Needs manual visual pass | Automation opened Settings but did not switch tab reliably |
| Probe/Setup/Smoke controls present in source | Pass | Implemented in Settings plugin tab |

## Manual visual pass still recommended

With dev app running, manually confirm:

1. Open Settings.
2. Click `插件` in the left Settings sidebar.
3. Confirm `SA-LUT Color` and `NAFNet Restore` both appear.
4. Expand `NAFNet Restore`.
5. For profile `AMD ROCm` / `windows-amd-rocm`, run:
   - Probe Runtime
   - Run Setup
   - Smoke
6. From a real image, trigger:
   - Fast Denoise
   - NAFNet Deblur
   - NAFNet JPEG Repair
7. Confirm `PluginActionDialog` shows stage/progress/cancel/output actions.

## Decision

No new host contract issue was found in this pass. The remaining visual click-through is UI QA, not a reason to add a third plugin or reopen the v1 contract.
