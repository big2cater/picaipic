# Runtime Path & Package Versions on Profile — 2026-07-02

Handoff note for continuing this work in the VS Code Claude extension. The
desktop agent (Linux sandbox) finished the code edits but could not run
`cargo check` / `npm run build` (no cargo in the sandbox; the mounted
locales mirror was truncated even though the host files are intact). The
next step is to compile-verify and rebuild the release exe from VS Code,
where the extension runs against the local Windows toolchain directly.

## Background

The plugin runtime layer already resolves three tiers — `shared`,
`plugin-private`, `external` — but the Settings UI did not surface which
venv a profile actually resolved to, nor which key packages it contained.
This step makes the resolved runtime path and a condensed package-version
line visible directly on each plugin profile row, so the user can see at a
glance "which venv am I really using, and what's inside it." Stale-service
/ not-running process lifecycle behavior verified earlier on 2026-07-02 is
unaffected — no process or state-machine code was touched.

## Changes

### Backend — `src-tauri/src/t_plugin.rs`

1. Probe script `python_runtime_probe_script()` (around line 2989) now
   also emits `numpy`, `opencv-python` (imported via `cv2`), and `rawpy`
   into `result["packages"]`, reusing the existing `package_version()`
   helper. No new package is imported when unavailable; the failed import
   is reported as `{available: false, error: ...}` just like torch.

2. `PluginInstallProfileSummary` (around line 620) gained
   `#[serde(default)] pub resolved_runtime_dir: Option<String>`. The
   summary builder (around line 3571) computes it from the active binding
   (persisted `state.runtime_binding` → `profile.runtime_binding`):

   - `external` → `binding.root`, else parent of `binding.python`.
   - `shared` → `profile_runtime_root(plugin_id, effective_profile)`
     (i.e. `<store>/shared-runtimes/<binding.id || profile.id>`).
   - otherwise → `profile_runtime_dir` (which joins `env_dir`), falling
     back to `profile_runtime_root` if that returns `None`/errors.

   The field is `None` only when no binding and no python path exist.

### Frontend — `src-vite/src`

3. `common/pluginRuntime.ts` mirrored `resolvedRuntimeDir?: string | null`
   on `AiPluginInstallProfile`.

4. `views/Settings.vue`:

   - `runtimeProbeDetailGroups` `pkgOrder` (around line 3032) extended to
     `['torch', 'torchDirectML', 'onnxruntime', 'numpy', 'opencv-python',
     'rawpy']`. The Packages detail group iterates `data.packages`, so the
     three new packages surface automatically once the probe runs.
   - Added a runtime-path chip on the profile row, right after the
     binding badge (around line 924). It reuses
     `runtimeBindingBadgeClass` for tier color (shared → green,
     plugin-private → blue via `bg-info`, external → yellow), shows the
     last two path segments (e.g. `shared-runtimes\python312-rocm72-torch291`),
     has an "Open" button that calls `openPluginPath(profile.resolvedRuntimeDir)`,
     and appends a condensed version line when a cached probe exists,
     e.g. `Python 3.12 · torch 2.9.1 · numpy 1.26.4`. Full detail still
     flows through the existing probe card below.
   - New helpers: `profileRuntimePathChip`, `shortRuntimePath`,
     `condensedRuntimeVersions`. New i18n key `openRuntimeFolder` added
     to the inline fallback in `pluginText`, `locales/en.json`, and
     `locales/zh.json`.

## Verification still needed

Run these from the project root; the desktop sandbox could not:

```
cd src-tauri && cargo check
cd ../src-vite && npm run build
```

Then rebuild the release exe via the existing `build-exe.bat` and
re-test the earlier 2026-07-02 scenario: start SA-LUT → close Lap.exe →
reopen. Expected: process killed shows "not running"; deliberately left
old service shows "stale service" — same as before, since process /
state-machine code was not touched.

Sanity-check the new UI: open Settings → a plugin with a profile, confirm
the runtime-path chip shows the tier color and the last two segments of
the resolved venv, that "Open" reveals that folder, and that after
clicking Probe the chip appends `Python · torch · numpy` versions. For
`picai-salut-color` on the external runtime, the chip should show yellow
(external) and the parent of the venv python.

## Next planned step (not started)

Dependency-conflict hints: when a shared runtime already has e.g.
`numpy==1.26.4` but a plugin's `requirements` asks `numpy>=2`, surface a
hint like "consider a plugin-private runtime." This layers on top of the
probe `packages` + plugin `requirements` already wired here, so the path
chip from this step is the foundation. Not started; waiting on user
confirmation after this UI change.