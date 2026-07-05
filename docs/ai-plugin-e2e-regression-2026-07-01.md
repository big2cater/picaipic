# AI Plugin E2E Regression - 2026-07-01

## Scope

Real-machine Settings > Smoke regression on `picai-salut-color` after the
2026-07-01 release exe rebuild. The goal was to verify the host smoke path
end to end, not algorithm quality.

## Test environment

- Windows
- Plugin under test: `D:\ailab\PicAiPic\plugins\picai-salut-color` (development directory, not a zip install)
- Selected profile: `windows-amd-rocm` (backend `rocm`)
- External runtime used via `.local.env`:
  - Python: `D:\ailab\20260610133133\backend\venv\Scripts\python.exe`
  - Source: `D:\ailab\20260610133133\backend`
  - Models: `D:\ailab\20260610133133\backend\models\salut`
- GPU: AMD Radeon RX 7900 XT (ROCm PyTorch 2.9.1+rocm7.2.1)

## Symptoms and root causes

Clicking Smoke on the SA-LUT card returned:

```text
error sending request for url (http://127.0.0.1:8011/health)
```

The error came from `wait_for_plugin_ready` in `t_plugin.rs` polling
`/health` after `start.bat` had already exited. Two independent defects in
`start.bat` caused the backend to never listen.

### Defect 1: LF line endings on .bat files

All plugin `.bat` files (`start.bat`, `stop.bat`, `install.bat` for both
`picai-salut-color` and `picai-nafnet-restore`) had been saved with Unix LF
line endings. `cmd.exe` rejects LF batch files containing complex constructs
with:

```text
The syntax of the command is incorrect.
```

This was visible in `plugins/picai-salut-color/logs/start.log` across three
spawn attempts on 2026-07-01 11:23-11:25. Confirmed by `file start.bat`
(showed `data` / no `CRLF` claim) and `grep -c $'\r'` returning 0.

Fix: converted all six plugin `.bat` files to CRLF. There is no
`.gitattributes` because this is a local tree, not a git repository; the
discipline is "never re-save `.bat` files as LF". The packaging script
`package_plugin.ps1` copies files as-is, so the LF bug would have shipped in
the release zips (`dist/plugins/*.zip` from 2026-06-30 were confirmed to
contain LF `start.bat`).

### Defect 2: nested `%%A:~0,1%` substring if inside `for /f`

The `.local.env` loader in both `start.bat` files used:

```bat
for /f "usebackq tokens=1,* delims==" %%A in (".local.env") do (
  if not "%%A"=="" if not "%%A:~0,1%"=="#" set "%%A=%%B"
)
```

`cmd.exe` on this machine rejects the nested `%%A:~0,1%` substring comparison
inside the `if`. Reproduced by running `start.bat` directly in a clean
`cmd.exe` window: still `The syntax of the command is incorrect.` even after
the CRLF fix. `picai-nafnet-restore` had the identical pattern; it had never
been spawned (no `start.log`), so the bug was latent there.

Fix: use the `eol=#` option on `for /f` to skip comment lines instead of a
runtime substring check:

```bat
for /f "usebackq eol=# tokens=1,* delims==" %%A in (".local.env") do (
  if not "%%A"=="" set "%%A=%%B"
)
```

Applied to both `plugins/picai-salut-color/start.bat` and
`plugins/picai-nafnet-restore/start.bat`.

### Defect 3 (after 1+2 fixed): wrong venv selected

After the `start.bat` syntax fixes, the backend started but Smoke failed at
the torch step with `No module named 'torch'`. `start.bat` had picked the
empty `.venv\Scripts\python.exe` (created by an earlier setup attempt, no
PyTorch installed) instead of the machine's real ROCm PyTorch venv.

Fix: added `plugins/picai-salut-color/.local.env` pointing
`PICAIPIC_SALUT_PYTHON`, `SALUT_SOURCE_DIR`, and `SALUT_MODEL_DIR` at the
external runtime. `start.bat` reads `.local.env` first, so the external venv
wins over the empty `.venv`. `.local.env` is excluded from release packages
by `package_plugin.ps1`.

Changed files:

- `plugins/picai-salut-color/start.bat`
- `plugins/picai-salut-color/stop.bat`
- `plugins/picai-salut-color/install.bat`
- `plugins/picai-nafnet-restore/start.bat`
- `plugins/picai-nafnet-restore/stop.bat`
- `plugins/picai-nafnet-restore/install.bat`
- `plugins/picai-salut-color/.local.env` (new; not packaged)

## Results

| Case | Result | Notes |
| --- | --- | --- |
| `start.bat` manual run | Pass | `picai-salut-color listening on 127.0.0.1:8011`, no syntax error |
| `netstat -ano \| findstr :8011` after start | Pass | TCP LISTENING, PID present |
| Settings > Smoke (SA-LUT, rocm profile) | Pass | `passed=true`, profile marked verified |
| `POST /smoke-test` | Pass | 200, `passed=true` |

NAFNet was not re-smoked in this session; its `start.bat` received the same
two fixes preventively because it carried the identical `for /f` pattern and
had never been spawned before.

## Contract observations

- A `start.bat` that exits before listening is currently surfaced to the user
  only as `error sending request for url (.../health)`, with no link to
  `start.log` and no remediation advice. This is a real UX gap, not just a
  plugin authoring issue, because the failure mode is indistinguishable from
  "backend slow to start" or "port blocked" until the user reads
  `start.log` manually. Tracked as the next work item in
  `docs/ai-plugin-current-status.md` Next work #5.
- `.local.env` is the correct and supported mechanism for reusing an
  already-installed external Python/PyTorch runtime. It should not be
  replaced by machine-specific paths in release manifests.
- Release plugin zips (`dist/plugins/*.zip`) from before this session
  contained LF `start.bat` files and would reproduce Defect 1 on a fresh
  install. Regenerating the zips with `.\package-plugins.bat` is required
  before any further distribution.

## Follow-up

1. Regenerate plugin zips: `.\package-plugins.bat` (the on-disk `.bat` files
   are now CRLF and the `for /f` syntax is fixed).
2. Add the "startup failure actionable UX" item to the v1-hardening backlog
   and implement it: detect early `start.bat` exit, surface `start.log`
   tail in Settings, emit structured startup-failure advice.
3. Consider adding an `external` runtimeBinding with an empty `python` field
   to rocm/cuda profiles so Settings can offer "point at an existing venv"
   to users who have one, without writing machine paths into the release
   manifest. Not done this session.