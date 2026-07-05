# AI Plugin E2E Regression - 2026-06-30

## Scope

Validated the current host plugin contract using the two real plugins:

- `picai-nafnet-restore`
- `picai-salut-color`

The purpose was to verify the host/plugin interface, not algorithm quality.

## Test environment

- Windows
- Python: `D:\ailab\20260610133133\backend\venv\Scripts\python.exe`
- GPU reported by PyTorch ROCm runtime: AMD Radeon RX 7900 XT
- NAFNet temp port: 18112
- SA-LUT temp ports: 18111, 18113 after smoke fix

## Results

| Case | Result | Notes |
| --- | --- | --- |
| NAFNet `/health` | Pass | Ready true |
| NAFNet `/status` | Pass | Models present, torch available |
| SA-LUT `/health` | Pass | Ready true |
| SA-LUT `/status` | Pass | Models present, torch available |
| NAFNet smoke denoise | Pass | Model load succeeded |
| SA-LUT smoke color-transfer | Initially failed, then fixed | Tiny smoke input was 16x16 and invalid for model padding; changed to 128x128 |
| NAFNet fast denoise task | Pass | `method=opencv-fast`, no slow denoise path |
| NAFNet mock cancel task | Pass | `cancelling` -> `cancelled`, error domain `task` |
| NAFNet invalid input failure | Pass | Structured failure returned; domain currently `plugin` |
| SA-LUT color-transfer task | Pass | Real async invoke succeeded and produced output |

## Issues found

### SA-LUT smoke input too small

SA-LUT smoke used a 16x16 synthetic image. The underlying model applies padding that requires the input dimension to be larger than the padding. Smoke failed with:

```text
Padding size should be less than the corresponding input dimension
```

Real `color-transfer` invocation with normal image sizes worked. The smoke test was fixed by changing the synthetic image to 128x128.

Changed file:

- `D:\ailab\PicAiPic\plugins\picai-salut-color\backend\salut_adapter.py`

## Contract observations

- Smoke tests must use representative minimum input sizes for the plugin/model.
- A plugin may be operational even if smoke is incorrectly authored; smoke failures should provide structured JSON details.
- Async task lifecycle is working across both plugins.
- Cancellation and failure error domains are visible in the UI path.
- NAFNet remains useful as a heavyweight backend stress test, not as a denoise quality benchmark.
- No third plugin is needed before v1 freeze; the two existing plugins cover normal and heavyweight local HTTP workflows.

## Follow-up decisions

- Generic manifest timeout hints are deferred to v1.1; v1 keeps timeouts as host policy except `smokeTest.timeoutMs`.
- Contract language has been tightened in `D:\ailab\PicAiPic\docs\ai-plugin-contract-v1-draft.md`.
- A plugin author checklist has been added at `D:\ailab\PicAiPic\docs\ai-plugin-author-checklist.md`.

## Artifacts

Generated temporary files under:

- `D:\ailab\PicAiPic\tmp_plugin_e2e`

Initial machine-readable report:

- `D:\ailab\PicAiPic\tmp_plugin_e2e\e2e-report.json`
