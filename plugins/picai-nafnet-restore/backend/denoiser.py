from __future__ import annotations

import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import cv2
import numpy as np


DEFAULT_NAFNET_ROOT = Path(
    os.environ.get("NAFNET_SOURCE_DIR")
    or Path(os.environ.get("PICAIPIC_PLUGIN_ROOT", ".")).joinpath("models", "nafnet")
)
DEBLUR_MAX_SIDE = int(os.environ.get("NAFNET_DEBLUR_MAX_SIDE") or "1600")
JPEG_MAX_SIDE = int(os.environ.get("NAFNET_JPEG_MAX_SIDE") or "1600")
DENOISE_MAX_FULL_PIXELS = int(os.environ.get("NAFNET_DENOISE_MAX_FULL_PIXELS") or "8000000")
OPENCV_DENOISE_MAX_SIDE = int(os.environ.get("PICAIPIC_OPENCV_DENOISE_MAX_SIDE") or "1800")


@dataclass(frozen=True)
class NAFNetTask:
    key: str
    label: str
    weights: str
    width: int
    enc_blk_nums: tuple[int, ...]
    middle_blk_num: int
    dec_blk_nums: tuple[int, ...]
    local: bool = False


NAFNET_TASKS: dict[str, NAFNetTask] = {
    "denoise": NAFNetTask(
        key="denoise",
        label="NAFNet SIDD Denoise",
        weights="NAFNet-SIDD-width64.pth",
        width=64,
        enc_blk_nums=(2, 2, 4, 8),
        middle_blk_num=12,
        dec_blk_nums=(2, 2, 2, 2),
    ),
    "deblur": NAFNetTask(
        key="deblur",
        label="NAFNet GoPro Deblur",
        weights="NAFNet-GoPro-width64.pth",
        width=64,
        enc_blk_nums=(1, 1, 1, 28),
        middle_blk_num=1,
        dec_blk_nums=(1, 1, 1, 1),
        local=True,
    ),
    "jpeg": NAFNetTask(
        key="jpeg",
        label="NAFNet REDS JPEG Artifact Removal",
        weights="NAFNet-REDS-width64.pth",
        width=64,
        enc_blk_nums=(1, 1, 1, 28),
        middle_blk_num=1,
        dec_blk_nums=(1, 1, 1, 1),
        local=True,
    ),
}


class NAFNetRestorer:
    def __init__(self, source_root: str | Path | None = None) -> None:
        self.source_root = Path(source_root) if source_root else DEFAULT_NAFNET_ROOT
        self.models: dict[str, Any] = {}
        self.device: str = "cpu"
        self.errors: dict[str, str] = {}
        self.last_debug: dict[str, Any] = {}

    def weights_path(self, task: str) -> Path:
        task_def = NAFNET_TASKS[task]
        return self.source_root / "experiments" / "pretrained_models" / task_def.weights

    def task_status(self, task: str) -> dict[str, Any]:
        task_def = NAFNET_TASKS[task]
        weights_path = self.weights_path(task)
        return {
            "label": task_def.label,
            "weightsPath": str(weights_path),
            "weightsExists": weights_path.exists(),
            "loaded": task in self.models,
            "error": self.errors.get(task),
        }

    def status(self) -> dict[str, Any]:
        return {
            "sourceDir": str(self.source_root),
            "sourceExists": self.source_root.exists(),
            "device": self.device,
            "tasks": {task: self.task_status(task) for task in NAFNET_TASKS},
        }

    def available(self, task: str) -> bool:
        return task in NAFNET_TASKS and self.source_root.exists() and self.weights_path(task).exists()

    def load(self, task: str, requested_device: str = "auto") -> None:
        if task in self.models:
            return
        if task not in NAFNET_TASKS:
            raise ValueError(f"Unknown NAFNet task: {task}")
        if not self.available(task):
            message = f"NAFNet weights not found: {self.weights_path(task)}"
            self.errors[task] = message
            raise FileNotFoundError(message)

        try:
            import torch

            if str(self.source_root) not in sys.path:
                sys.path.insert(0, str(self.source_root))

            from basicsr.models.archs.NAFNet_arch import NAFNet, NAFNetLocal

            task_def = NAFNET_TASKS[task]
            self.device = self._resolve_device(torch, requested_device)
            model_class = NAFNetLocal if task_def.local else NAFNet
            model_kwargs: dict[str, Any] = {
                "img_channel": 3,
                "width": task_def.width,
                "enc_blk_nums": list(task_def.enc_blk_nums),
                "middle_blk_num": task_def.middle_blk_num,
                "dec_blk_nums": list(task_def.dec_blk_nums),
            }
            if task_def.local:
                model_kwargs["train_size"] = (1, 3, 256, 256)
            model = model_class(**model_kwargs)
            checkpoint = torch.load(str(self.weights_path(task)), map_location="cpu")
            state = checkpoint
            for key in ("params", "params_ema", "state_dict"):
                if isinstance(checkpoint, dict) and key in checkpoint:
                    state = checkpoint[key]
                    break
            if isinstance(state, dict):
                state = {
                    key.replace("module.", "", 1).replace("net_g.", "", 1): value
                    for key, value in state.items()
                }
            model.load_state_dict(state, strict=True)
            model.eval().to(self.device)
            self.models[task] = model
            self.errors.pop(task, None)
        except Exception as exc:
            self.errors[task] = str(exc)
            self.models.pop(task, None)
            raise

    def restore(
        self,
        img_bgr: np.ndarray,
        task: str,
        requested_device: str = "auto",
        tile_size: int = 1024,
        overlap: int = 128,
    ) -> np.ndarray:
        self.load(task, requested_device=requested_device)
        model = self.models[task]

        import torch

        original_h, original_w = img_bgr.shape[:2]
        model_img = img_bgr
        scale = 1.0
        max_model_side = None
        if task == "deblur":
            max_model_side = DEBLUR_MAX_SIDE
        elif task == "jpeg":
            max_model_side = JPEG_MAX_SIDE

        if max_model_side is not None:
            max_side = max(original_h, original_w)
            if max_side > max_model_side:
                scale = max_model_side / float(max_side)
                resized_w = max(8, int(round(original_w * scale)))
                resized_h = max(8, int(round(original_h * scale)))
                model_img = cv2.resize(img_bgr, (resized_w, resized_h), interpolation=cv2.INTER_AREA)

        img_rgb = cv2.cvtColor(model_img, cv2.COLOR_BGR2RGB).astype(np.float32) / 255.0
        tensor = torch.from_numpy(img_rgb.transpose(2, 0, 1)).unsqueeze(0).to(self.device)
        tiled = False
        safe_tiled = False
        with torch.no_grad():
            if task == "denoise":
                # NAFNet SIDD is not safe to run patch-wise on large real-world photos:
                # patch inference can introduce local color/exposure shifts and checkerboard
                # chroma artifacts. For images that are too large for safe full-frame
                # inference, let restore_image(method="auto") fall back to OpenCV instead
                # of returning a corrupted NAFNet result.
                if original_h * original_w > DENOISE_MAX_FULL_PIXELS:
                    raise RuntimeError(
                        f"NAFNet denoise full-frame limit exceeded: {original_w}x{original_h} > {DENOISE_MAX_FULL_PIXELS} pixels"
                    )
                output = self._forward_padded(model, tensor)
            elif task == "deblur":
                output = self._forward_padded(model, tensor)
            elif max(model_img.shape[:2]) <= tile_size:
                output = self._forward_padded(model, tensor)
            else:
                tiled = True
                output = self._tile_forward(model, tensor, tile_size=tile_size, overlap=overlap)
        out = output.squeeze(0).detach().cpu().clamp(0, 1).numpy().transpose(1, 2, 0)
        out_bgr = cv2.cvtColor((out * 255.0).round().astype(np.uint8), cv2.COLOR_RGB2BGR)
        if scale < 1.0:
            out_bgr = cv2.resize(out_bgr, (original_w, original_h), interpolation=cv2.INTER_CUBIC)
        if has_restoration_artifacts(out_bgr):
            raise RuntimeError("NAFNet output artifact check failed")
        self.last_debug = {
            "device": self.device,
            "inputSize": [int(original_w), int(original_h)],
            "modelSize": [int(model_img.shape[1]), int(model_img.shape[0])],
            "scale": round(float(scale), 4),
            "maxSide": max_model_side,
            "tiled": tiled,
            "safeTiled": safe_tiled,
        }
        return out_bgr

    def _resolve_device(self, torch: Any, requested_device: str) -> str:
        requested = (requested_device or "auto").lower().strip()
        if requested == "cpu":
            return "cpu"
        if requested in {"cuda", "rocm", "auto"} and torch.cuda.is_available():
            return "cuda"
        return "cpu"

    def _forward_padded(self, model: Any, tensor: Any) -> Any:
        import torch.nn.functional as F

        _, _, h, w = tensor.shape
        pad_h = (8 - h % 8) % 8
        pad_w = (8 - w % 8) % 8
        if pad_h or pad_w:
            tensor = F.pad(tensor, (0, pad_w, 0, pad_h), mode="reflect")
        output = model(tensor)
        if isinstance(output, list):
            output = output[-1]
        return output[:, :, :h, :w]

    def _is_large_tensor_runtime_error(self, exc: RuntimeError) -> bool:
        message = str(exc).lower()
        return (
            "canuse32bitindexmath" in message
            or "out of memory" in message
            or "hip_out_of_memory" in message
            or "cuda out of memory" in message
        )

    def _denoise_tile_forward(self, model: Any, tensor: Any, tile_size: int, overlap: int) -> Any:
        import torch

        _, _, h, w = tensor.shape
        tile_size = max(512, int(tile_size))
        context = max(64, min(int(overlap), tile_size // 2))
        core = max(256, tile_size - 2 * context)
        output = torch.zeros_like(tensor)
        weight = torch.zeros((1, 1, h, w), device=tensor.device, dtype=tensor.dtype)

        ys = list(range(0, h, core))
        xs = list(range(0, w, core))
        if ys and ys[-1] >= h:
            ys.pop()
        if xs and xs[-1] >= w:
            xs.pop()

        for y0 in ys:
            y1 = min(y0 + core, h)
            py0 = max(0, y0 - context)
            py1 = min(h, y1 + context)
            for x0 in xs:
                x1 = min(x0 + core, w)
                px0 = max(0, x0 - context)
                px1 = min(w, x1 + context)
                patch = tensor[:, :, py0:py1, px0:px1]
                restored = self._forward_padded(model, patch)

                # Denoising should not introduce broad local exposure/color shifts. NAFNet
                # SIDD can do that when evaluated patch-wise on large real-world JPEGs, so
                # keep high-frequency restoration while anchoring each patch's low-frequency
                # color to its input patch.
                restored = self._match_patch_low_frequency(restored, patch)

                cy0, cy1 = y0 - py0, y1 - py0
                cx0, cx1 = x0 - px0, x1 - px0
                core_out = restored[:, :, cy0:cy1, cx0:cx1]
                blend = self._tile_blend_window(
                    y1 - y0,
                    x1 - x0,
                    device=tensor.device,
                    dtype=tensor.dtype,
                    top_edge=y0 == 0,
                    left_edge=x0 == 0,
                    bottom_edge=y1 >= h,
                    right_edge=x1 >= w,
                    overlap=min(context, core // 2),
                )
                output[:, :, y0:y1, x0:x1] += core_out * blend
                weight[:, :, y0:y1, x0:x1] += blend

        return output / weight.clamp_min(1e-6)

    def _match_patch_low_frequency(self, restored: Any, source: Any) -> Any:
        import torch.nn.functional as F

        _, _, h, w = restored.shape
        # Pool over a coarse grid rather than one global mean, preserving legitimate local
        # lighting gradients while removing patch-specific color/exposure bias.
        grid_h = max(1, min(8, h // 128))
        grid_w = max(1, min(8, w // 128))
        restored_low = F.adaptive_avg_pool2d(restored, (grid_h, grid_w))
        source_low = F.adaptive_avg_pool2d(source, (grid_h, grid_w))
        bias = F.interpolate(restored_low - source_low, size=(h, w), mode="bilinear", align_corners=False)
        return (restored - bias).clamp(0, 1)
    def _tile_forward(self, model: Any, tensor: Any, tile_size: int, overlap: int) -> Any:
        import torch

        _, _, h, w = tensor.shape
        tile_size = max(128, int(tile_size))
        overlap = max(0, min(int(overlap), tile_size - 1))
        stride = max(1, tile_size - overlap)
        output = torch.zeros_like(tensor)
        weight = torch.zeros((1, 1, h, w), device=tensor.device, dtype=tensor.dtype)

        ys = list(range(0, max(h - tile_size, 0) + 1, stride))
        xs = list(range(0, max(w - tile_size, 0) + 1, stride))
        if not ys or ys[-1] != max(h - tile_size, 0):
            ys.append(max(h - tile_size, 0))
        if not xs or xs[-1] != max(w - tile_size, 0):
            xs.append(max(w - tile_size, 0))

        for y in ys:
            for x in xs:
                patch = tensor[:, :, y : min(y + tile_size, h), x : min(x + tile_size, w)]
                restored = self._forward_padded(model, patch)
                ph, pw = restored.shape[-2:]
                blend = self._tile_blend_window(
                    ph,
                    pw,
                    device=tensor.device,
                    dtype=tensor.dtype,
                    top_edge=y == 0,
                    left_edge=x == 0,
                    bottom_edge=y + ph >= h,
                    right_edge=x + pw >= w,
                    overlap=overlap,
                )
                output[:, :, y : y + ph, x : x + pw] += restored * blend
                weight[:, :, y : y + ph, x : x + pw] += blend

        return output / weight.clamp_min(1e-6)

    def _tile_blend_window(
        self,
        height: int,
        width: int,
        *,
        device: Any,
        dtype: Any,
        top_edge: bool,
        left_edge: bool,
        bottom_edge: bool,
        right_edge: bool,
        overlap: int,
    ) -> Any:
        import torch

        if overlap <= 0:
            return torch.ones((1, 1, height, width), device=device, dtype=dtype)
        ramp_h = max(1, min(overlap, height // 2))
        ramp_w = max(1, min(overlap, width // 2))
        wy = torch.ones(height, device=device, dtype=dtype)
        wx = torch.ones(width, device=device, dtype=dtype)
        if not top_edge and ramp_h > 0:
            wy[:ramp_h] = torch.linspace(1.0 / (ramp_h + 1), 1.0, ramp_h, device=device, dtype=dtype)
        if not bottom_edge and ramp_h > 0:
            wy[-ramp_h:] = torch.minimum(
                wy[-ramp_h:],
                torch.linspace(1.0, 1.0 / (ramp_h + 1), ramp_h, device=device, dtype=dtype),
            )
        if not left_edge and ramp_w > 0:
            wx[:ramp_w] = torch.linspace(1.0 / (ramp_w + 1), 1.0, ramp_w, device=device, dtype=dtype)
        if not right_edge and ramp_w > 0:
            wx[-ramp_w:] = torch.minimum(
                wx[-ramp_w:],
                torch.linspace(1.0, 1.0 / (ramp_w + 1), ramp_w, device=device, dtype=dtype),
            )
        return wy.view(1, 1, height, 1) * wx.view(1, 1, 1, width)


def opencv_denoise(
    img_bgr: np.ndarray,
    strength: float = 0.55,
    detail: float = 0.65,
    sharpen: float = 0.15,
) -> np.ndarray:
    strength = float(np.clip(strength, 0.0, 1.0))
    detail = float(np.clip(detail, 0.0, 1.0))
    sharpen = float(np.clip(sharpen, 0.0, 1.0))

    original_h, original_w = img_bgr.shape[:2]
    work = img_bgr
    scale = 1.0
    max_side = max(original_h, original_w)
    if max_side > OPENCV_DENOISE_MAX_SIDE:
        scale = OPENCV_DENOISE_MAX_SIDE / float(max_side)
        work_w = max(8, int(round(original_w * scale)))
        work_h = max(8, int(round(original_h * scale)))
        work = cv2.resize(img_bgr, (work_w, work_h), interpolation=cv2.INTER_AREA)

    # Always use the fast path for plugin UX. cv2.fastNlMeansDenoisingColored is
    # high quality but can block for a long time on camera-sized photos.
    denoised = cv2.bilateralFilter(
        work,
        d=5,
        sigmaColor=18 + strength * 42,
        sigmaSpace=6 + strength * 14,
    )

    if scale < 1.0:
        denoised = cv2.resize(denoised, (original_w, original_h), interpolation=cv2.INTER_CUBIC)

    if detail > 0:
        blend = min(0.45, detail * 0.45)
        denoised = cv2.addWeighted(denoised, 1.0 - blend, img_bgr, blend, 0)

    if sharpen > 0:
        denoised = unsharp_mask(denoised, sharpen)

    return np.clip(denoised, 0, 255).astype(np.uint8)

def opencv_jpeg_artifact_reduce(img_bgr: np.ndarray, strength: float = 0.45, detail: float = 0.6) -> np.ndarray:
    strength = float(np.clip(strength, 0.0, 1.0))
    detail = float(np.clip(detail, 0.0, 1.0))

    filtered = cv2.bilateralFilter(
        img_bgr,
        d=5,
        sigmaColor=20 + strength * 45,
        sigmaSpace=8 + strength * 18,
    )
    if detail > 0:
        filtered = cv2.addWeighted(filtered, 1.0 - detail * 0.25, img_bgr, detail * 0.25, 0)
    return filtered.astype(np.uint8)


def unsharp_mask(img_bgr: np.ndarray, amount: float) -> np.ndarray:
    if amount <= 0:
        return img_bgr
    blur = cv2.GaussianBlur(img_bgr, (0, 0), 1.2)
    return np.clip(cv2.addWeighted(img_bgr, 1.0 + amount, blur, -amount, 0), 0, 255).astype(np.uint8)


def has_restoration_artifacts(img_bgr: np.ndarray) -> bool:
    small = cv2.resize(img_bgr, (128, 128), interpolation=cv2.INTER_AREA)
    b, g, r = cv2.split(small.astype(np.int16))
    chroma_spread = np.maximum.reduce([np.abs(r - g), np.abs(g - b), np.abs(b - r)])
    return float(np.mean(chroma_spread > 170)) > 0.22


def restore_image(
    img_bgr: np.ndarray,
    restorer: NAFNetRestorer,
    task: str = "denoise",
    method: str = "auto",
    device: str = "auto",
    strength: float = 0.55,
    detail: float = 0.65,
    sharpen: float = 0.15,
) -> tuple[np.ndarray, dict[str, Any]]:
    task = (task or "denoise").lower().strip()
    method = (method or "auto").lower().strip()
    if task not in NAFNET_TASKS:
        raise ValueError(f"Unknown restoration task: {task}")

    pixel_count = int(img_bgr.shape[0] * img_bgr.shape[1])

    # Product default: denoise must be fast and non-blocking. NAFNet SIDD is kept
    # as an explicit high-quality/small-image path only; it is not safe as the
    # default for large real-world photos on the current ROCm stack.
    if task == "denoise" and method in {"auto", "opencv", ""}:
        fallback = fallback_restore(img_bgr, task, strength=strength, detail=detail, sharpen=sharpen)
        return fallback, {
            "method": "opencv-fast",
            "task": task,
            "nafnetSkipped": "denoise auto uses fast OpenCV path to avoid slow or unstable NAFNet SIDD inference",
            "inputPixels": pixel_count,
            **restorer.status(),
        }

    if task == "denoise" and method == "nafnet" and pixel_count > DENOISE_MAX_FULL_PIXELS:
        raise RuntimeError(
            f"NAFNet denoise is disabled for large images: {img_bgr.shape[1]}x{img_bgr.shape[0]} "
            f"({pixel_count} pixels) > {DENOISE_MAX_FULL_PIXELS}. Use method=auto/opencv for fast denoise."
        )

    use_nafnet = method == "nafnet" or (method == "auto" and task in {"deblur"})

    if use_nafnet and restorer.available(task):
        try:
            return restorer.restore(img_bgr, task, requested_device=device), {
                "method": "nafnet",
                "task": task,
                "nafnetDebug": restorer.last_debug,
                **restorer.status(),
            }
        except Exception as exc:
            if method == "nafnet":
                raise
            fallback = fallback_restore(img_bgr, task, strength=strength, detail=detail, sharpen=sharpen)
            return fallback, {"method": "opencv", "task": task, "nafnetError": str(exc), **restorer.status()}

    if method == "nafnet":
        raise FileNotFoundError(f"NAFNet weights not found: {restorer.weights_path(task)}")

    return (
        fallback_restore(img_bgr, task, strength=strength, detail=detail, sharpen=sharpen),
        {"method": "opencv", "task": task, **restorer.status()},
    )

def fallback_restore(
    img_bgr: np.ndarray,
    task: str,
    strength: float = 0.55,
    detail: float = 0.65,
    sharpen: float = 0.15,
) -> np.ndarray:
    if task == "denoise":
        return opencv_denoise(img_bgr, strength=strength, detail=detail, sharpen=sharpen)
    if task == "jpeg":
        return opencv_jpeg_artifact_reduce(img_bgr, strength=strength, detail=detail)
    raise ValueError(f"Unknown restoration task: {task}")










