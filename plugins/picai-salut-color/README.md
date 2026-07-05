# picai-salut-color

PicAiPic plugin wrapper for SA-LUT color transfer. Packaged installs keep
replaceable plugin code separate from persistent models, logs, outputs, and
runtimes. Local development overrides belong in `.local.env`.

```text
backend
backend\engine
plugin-data\picai-salut-color\models\salut
```

This plugin exposes:

- `GET /health`
- `GET /status`
- `GET /diagnostics`
- `GET /tasks/{taskId}`
- `GET /tasks/{taskId}/events`
- `POST /invoke/color-transfer`
- `POST /invoke/export-lut`
- `POST /tasks/{taskId}/cancel`

Current state:

- The local HTTP plugin contract is implemented.
- `color-transfer` uses async invoke: `POST /invoke/color-transfer` returns
  `202 Accepted` with a `taskId`, then a single background worker runs the
  actual task.
- The worker exposes task status and long-poll task events for host tracking.
- `color-transfer` is wired to `engine.salut.SALUTInference` for real work.
- The package includes the SA-LUT `engine` subset used by the adapter, so zip
  installs do not require the old local source tree to import `engine`.
- Real `color-transfer` emits progress around model load, image reads,
  inference boundaries, output encode/write, and finalization. Cancellation is
  cooperative and cannot hard-interrupt a blocking model call already in flight.
- JPG/PNG/TIFF-like inputs are read through OpenCV. RAW inputs such as `.RW2`,
  `.CR2`, `.CR3`, `.NEF`, `.DNG`, `.ARW`, `.ORF`, and `.RAF` are decoded through
  `rawpy` first. OpenCV is only used for image IO, resize/channel conversion,
  and output encoding; the color transfer itself is SA-LUT model inference.
- `parameters.mockTask=true` runs a cancellable sleep-based mock task for
  queue/event/cancel stress testing without loading SA-LUT.
- Task state history is bounded by `PICAIPIC_TASK_HISTORY_LIMIT` and defaults
  to 500 records.
- The host validates successful output paths, keeps successful task outputs
  until import/adopt/discard, and expires unadopted successful outputs with a
  TTL cleanup that marks them `discarded`.
- Settings shows recent tasks with status badges, progress, output counts, and
  retry/cancel/discard actions.
- `start.bat` prefers `PICAIPIC_SALUT_PYTHON` from `.local.env`, then the
  selected PicAiPic runtime environment, then local development venvs.
- Real inference has been verified with AMD ROCm PyTorch:
  `torch 2.9.1+rocm7.2.1`.
- ROCm appears as the `cuda` device string inside PyTorch. The UI can request
  `rocm`, and the adapter maps it to `cuda`.
- `export-lut` is declared in the host contract but is still `not_implemented`.

Expected model files:

```text
<plugin-store>\plugin-data\picai-salut-color\models\salut\vgg_normalised.pth
<plugin-store>\plugin-data\picai-salut-color\models\salut\epoch=100-step=4127466.ckpt.state.pt
```

Useful environment overrides:

```text
PICAIPIC_WINDOWS_SALUT_BACKEND
SALUT_SOURCE_DIR
SALUT_CKPT_PATH
SALUT_VGG_PATH
SALUT_FORCE_CPU
PICAIPIC_PLUGIN_PORT
```

Setup environment supplied by PicAiPic:

```text
PICAIPIC_PLUGIN_ID
PICAIPIC_PLUGIN_PROFILE_ID
PICAIPIC_PLUGIN_BACKEND
PICAIPIC_PLUGIN_CAPABILITY
PICAIPIC_PLUGIN_ROOT
PICAIPIC_PLUGIN_RUNTIME_SCOPE
PICAIPIC_PLUGIN_RUNTIME_KIND
PICAIPIC_PLUGIN_RUNTIME_ID
PICAIPIC_PLUGIN_RUNTIME_ROOT
PICAIPIC_PLUGIN_PYTHON
PICAIPIC_PLUGIN_ENV_DIR
PICAIPIC_PLUGIN_ENV_PATH
PICAIPIC_PLUGIN_REQUIREMENTS
PICAIPIC_PLUGIN_REQUIREMENTS_PATH
```

The CUDA/ROCm/DirectML/CPU profiles use shared runtime bindings for packaged
installs. PicAiPic's Run setup action creates or reuses the selected shared
venv under:

```text
<plugin-store>\shared-runtimes\<runtime-id>\Scripts\python.exe
```

The profile requirements install the matching torch runtime:

- ROCm: `python312-rocm72-torch291`
- CUDA: `python312-cuda121-torch231`
- CPU: `python312-cpu-torch231`
- DirectML: `python312-directml`

Setup downloads are explicit manifest permissions and are limited to the
declared dependency domains. The setup script verifies imports after
installation (`torch`, `numpy`, `cv2`, `rawpy`, and `torch_directml` for the
DirectML profile). Local development can still point at an existing runtime by
copying `.local.env.example` to `.local.env` and setting
`PICAIPIC_SALUT_PYTHON`, `SALUT_SOURCE_DIR`, `SALUT_MODEL_DIR`,
`SALUT_CKPT_PATH`, or `SALUT_VGG_PATH`. `.local.env` is excluded from plugin
packages. A successful setup still requires PicAiPic's Smoke button before the
profile is considered verified.

Register this directory in PicAiPic plugin settings during development, or
install the generated zip from `dist\plugins\`:

```text
plugins\picai-salut-color
```

Useful async plumbing checks:

```text
python scripts\stress_salut_async.py --tasks 8 --duration-ms 250 --cancel-every 3
python scripts\stress_salut_http.py --tasks 6 --duration-ms 250 --cancel-every 3
```
