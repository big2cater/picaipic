# picai-nafnet-restore

PicAiPic plugin wrapper for the local NAFNet port.

The plugin exposes three image restoration capabilities:

- `denoise` -> fast OpenCV path by default; optional NAFNet SIDD with `method=nafnet`.
- `deblur` -> NAFNet GoPro.
- `jpeg-artifact-removal` -> NAFNet REDS, with OpenCV fallback.

Optional package-relative NAFNet source and weights:

```text
models\nafnet
models\nafnet\experiments\pretrained_models\NAFNet-SIDD-width64.pth
models\nafnet\experiments\pretrained_models\NAFNet-GoPro-width64.pth
models\nafnet\experiments\pretrained_models\NAFNet-REDS-width64.pth
```

Packaged installs can pass `denoise` Smoke without these files. The full
NAFNet checkout and weights are only required for explicit NAFNet model paths
such as `deblur` or `denoise` with `method=nafnet`.

Packaged installs use PicAiPic shared runtime bindings. Run setup creates or
reuses the selected shared venv under:

```text
<plugin-store>\shared-runtimes\<runtime-id>\Scripts\python.exe
```

The ROCm/CUDA/CPU profile requirements install the matching torch runtime and
then verify imports (`torch`, `numpy`, `cv2`, `skimage`, `timm`, `yaml`, and
`addict`). Setup downloads are explicit manifest permissions and are limited to
the declared dependency domains.

For local development, copy `.local.env.example` to `.local.env` and set
`PICAIPIC_NAFNET_PYTHON` or `NAFNET_SOURCE_DIR` there. `.local.env` is excluded
from plugin packages.

Endpoints:

- `GET /health`
- `GET /status`
- `GET /diagnostics`
- `GET /tasks/{taskId}`
- `GET /tasks/{taskId}/events`
- `POST /invoke/denoise`
- `POST /invoke/deblur`
- `POST /invoke/jpeg-artifact-removal`
- `POST /tasks/{taskId}/cancel`

Useful environment overrides:

```text
NAFNET_SOURCE_DIR
NAFNET_DEBLUR_MAX_SIDE
NAFNET_JPEG_MAX_SIDE
PICAIPIC_PLUGIN_PORT
PICAIPIC_TASK_HISTORY_LIMIT
```

Register this directory in PicAiPic plugin settings during development, or
install the generated zip from `dist\plugins\`:

```text
plugins\picai-nafnet-restore
```

Useful task-contract check:

```text
python scripts\stress_nafnet_http.py --tasks 6 --duration-ms 250 --cancel-every 3
```

The plugin has been smoke-checked with the existing ROCm runtime and a real
`denoise` invoke against `src-tauri/icons/icon.png`.
