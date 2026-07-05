"""
SA-LUT inference wrapper.
Two-stage pipeline for efficiency and full-resolution output:
  Stage 1: Run neural network at a moderate resolution to extract 4D LUT + context map
  Stage 2: Upscale context map to original resolution and apply 4D LUT at full resolution
This keeps VRAM usage low and is much faster, while preserving original image resolution.
"""
from pathlib import Path
from typing import Union

import cv2
import numpy as np
import torch
import torch.nn.functional as F

from engine.device_manager import resolve_device
from .model import VLog2StyleNet4D
from .pytorch_interpolation import trilinear_interpolate


class SALUTInference:
    """
    Wrapper around SA-LUT for easy inference.
    Uses a two-stage approach for full-resolution output:
      1. Neural network at low/mid resolution → extract LUT coefficients + context map
      2. Apply 4D LUT at original resolution (fast, no neural net needed)
    """

    def __init__(
        self,
        ckpt_path: str,
        vgg_path: str = "",
        standard_lut_path: str = "",
        device: Union[str, torch.device] = "auto",
        dim: int = 17,
        num_basis: int = 64,
    ):
        resolved_device = resolve_device(device) if isinstance(device, str) else device
        self.device = resolved_device if isinstance(resolved_device, torch.device) else torch.device(resolved_device)
        self.dim = dim
        self.num_basis = num_basis

        self.model = VLog2StyleNet4D(
            dim=dim,
            num_basis=num_basis,
            vgg_weight_path=vgg_path,
            standard_lut_path=standard_lut_path,
        ).to(self.device).eval()

        if Path(ckpt_path).exists():
            state = torch.load(ckpt_path, map_location=self.device, weights_only=False)
            # SA-LUT checkpoint may be a Lightning checkpoint or raw state_dict
            if isinstance(state, dict) and "state_dict" in state:
                state = state["state_dict"]
            # Strip common prefixes added by training frameworks
            stripped = {}
            for k, v in state.items():
                if k.startswith("vlog2stylenet."):
                    stripped[k[len("vlog2stylenet."):]] = v
                elif k.startswith("model."):
                    stripped[k[len("model."):]] = v
                else:
                    stripped[k] = v
            missing, unexpected = self.model.load_state_dict(stripped, strict=False)
            if missing:
                print(f"[SA-LUT] Missing keys ({len(missing)}): {missing[:5]}...")
            if unexpected:
                print(f"[SA-LUT] Unexpected keys ({len(unexpected)}): {unexpected[:5]}...")
            print(f"[SA-LUT] Loaded checkpoint from {ckpt_path}")
        else:
            print(f"[SA-LUT] Warning: checkpoint not found at {ckpt_path}; model will use random weights.")

    @torch.no_grad()
    def transfer(
        self,
        content_bgr: np.ndarray,
        style_bgr: np.ndarray,
        analysis_size: int = 1024,
    ) -> np.ndarray:
        """
        Transfer style from style_bgr to content_bgr.
        Input/output are BGR uint8 images at original resolution.

        Two-stage pipeline:
          1. Downscale images, run neural network to get 4D LUT + context map
          2. Upscale context map, apply 4D LUT at original resolution

        Args:
            content_bgr: Original content image (BGR, uint8)
            style_bgr: Style reference image (BGR, uint8)
            analysis_size: Resolution at which to run the neural network.
                           Higher = better style analysis but slower and more VRAM.
                           1024 is a good default for quality/speed balance.
        """
        orig_h, orig_w = content_bgr.shape[:2]
        print(f"[SA-LUT] Original resolution: {orig_w}x{orig_h}")

        # ── Stage 1: Neural network at analysis_size ──────────────────────
        content_small = self._resize_long_edge(content_bgr, analysis_size)
        style_small = self._resize_long_edge(style_bgr, analysis_size)

        fused_lut, context_map_small = self._extract_lut_and_context(
            content_small, style_small
        )
        print(f"[SA-LUT] LUT extracted at {content_small.shape[1]}x{content_small.shape[0]}")

        # ── Stage 2: Apply 4D LUT at full resolution ─────────────────────
        result = self._apply_lut_fullres(
            content_bgr, fused_lut, context_map_small,
            orig_h, orig_w,
        )
        print(f"[SA-LUT] LUT applied at full resolution: {orig_w}x{orig_h}")

        return result

    @torch.no_grad()
    def export_approx_cube_lut(
        self,
        content_bgr: np.ndarray,
        style_bgr: np.ndarray,
        output_path: str,
        size: int = 33,
        analysis_size: int = 1024,
    ) -> str:
        """
        Export an approximate 3D .cube LUT for this content/style pair.

        SA-LUT is content-adaptive: it predicts a 4D LUT plus a context map, so a
        plain 3D LUT cannot exactly reproduce spatially varying results. This
        method averages the context map for the current image and collapses the
        4D LUT into one 3D LUT that captures the overall grade.
        """
        content_small = self._resize_long_edge(content_bgr, analysis_size)
        style_small = self._resize_long_edge(style_bgr, analysis_size)
        fused_lut, context_map = self._extract_lut_and_context(content_small, style_small)

        context_weight = float(context_map[:, 0:1, :, :].mean().clamp(0, 1).item())
        lut_0 = fused_lut[0, :, 0, :, :, :]
        lut_1 = fused_lut[0, :, 1, :, :, :]
        lut = (lut_0 * (1.0 - context_weight) + lut_1 * context_weight).clamp(0, 1)

        if lut.shape[-1] != size:
            lut = F.interpolate(
                lut.unsqueeze(0),
                size=(size, size, size),
                mode="trilinear",
                align_corners=True,
            ).squeeze(0)

        lut_np = lut.detach().cpu().permute(1, 2, 3, 0).numpy()
        self._write_cube(lut_np, output_path, size)
        return output_path

    def _write_cube(self, lut_rgb: np.ndarray, output_path: str, size: int) -> None:
        path = Path(output_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            f.write("# AI Photo Color Match - SA-LUT approximate 3D LUT\n")
            f.write("# Collapsed from SA-LUT 4D LUT using the current image context average.\n")
            f.write(f"LUT_3D_SIZE {size}\n\n")
            for b in range(size):
                for g in range(size):
                    for r in range(size):
                        val = np.clip(lut_rgb[r, g, b], 0.0, 1.0)
                        f.write(f"{val[0]:.6f} {val[1]:.6f} {val[2]:.6f}\n")

    @torch.no_grad()
    def _extract_lut_and_context(
        self,
        content_bgr: np.ndarray,
        style_bgr: np.ndarray,
    ):
        """
        Run the SA-LUT neural network at the given resolution
        and return the fused 4D LUT and context map.
        """
        # Convert BGR -> RGB and normalize to [0, 1]
        content_rgb = cv2.cvtColor(content_bgr, cv2.COLOR_BGR2RGB).astype(np.float32) / 255.0
        style_rgb = cv2.cvtColor(style_bgr, cv2.COLOR_BGR2RGB).astype(np.float32) / 255.0

        # To tensor [1, C, H, W]
        content_t = torch.from_numpy(content_rgb.transpose(2, 0, 1)).unsqueeze(0)
        style_t = torch.from_numpy(style_rgb.transpose(2, 0, 1)).unsqueeze(0)

        # Pad to multiple of 32 for VGG
        h, w = content_t.shape[2], content_t.shape[3]
        pad_h = ((h + 31) // 32) * 32
        pad_w = ((w + 31) // 32) * 32
        if pad_h != h or pad_w != w:
            content_t = F.pad(content_t, (0, pad_w - w, 0, pad_h - h), mode="reflect")
            style_t = F.pad(style_t, (0, pad_w - w, 0, pad_h - h), mode="reflect")

        content_t = content_t.to(self.device)
        style_t = style_t.to(self.device)

        _, fused_lut, context_map = self.model(style_t, content_t)

        # Crop context_map back to unpadded size
        if pad_h != h or pad_w != w:
            context_map = context_map[:, :, :h, :w]

        return fused_lut, context_map

    def _apply_lut_fullres(
        self,
        content_bgr: np.ndarray,
        fused_lut: torch.Tensor,
        context_map_small: torch.Tensor,
        orig_h: int,
        orig_w: int,
    ) -> np.ndarray:
        """
        Apply the 4D LUT at the original full resolution.
        Upscales the context map and performs trilinear interpolation.
        This is a pure mathematical operation — no neural network, very fast.
        """
        # Upscale context map to original resolution (stays on same device as fused_lut)
        context_full = F.interpolate(
            context_map_small, size=(orig_h, orig_w), mode="bilinear", align_corners=True
        ).clamp(0, 1)

        # Prepare content tensor at full resolution and move to same device as LUT
        content_rgb = cv2.cvtColor(content_bgr, cv2.COLOR_BGR2RGB).astype(np.float32) / 255.0
        content_t = torch.from_numpy(content_rgb.transpose(2, 0, 1)).unsqueeze(0)  # [1, 3, H, W]
        content_t = content_t.to(fused_lut.device)

        # Apply 4D LUT: context_map selects between two 3D LUTs
        # fused_lut shape: [B, 3, N_ctx, D, D, D] where N_ctx=2
        context = context_full[:, 0:1, :, :]  # [1, 1, H, W]
        rgb = content_t.clamp(0, 1)            # [1, 3, H, W]

        # Process in tiles if image is very large to save VRAM
        result = self._apply_lut_tiled(fused_lut, context, rgb, orig_h, orig_w)

        # Convert back to numpy BGR with dithering to break 8-bit banding
        out_rgb = result.squeeze(0).permute(1, 2, 0).cpu().numpy()
        out_rgb = out_rgb * 255.0
        # Add blue-noise-like dithering (±0.6 uniform) to eliminate posterization bands
        dither = np.random.default_rng().uniform(-0.6, 0.6, out_rgb.shape)
        out_rgb = (out_rgb + dither).clip(0, 255).astype(np.uint8)
        out_bgr = cv2.cvtColor(out_rgb, cv2.COLOR_RGB2BGR)

        return out_bgr

    def _apply_lut_tiled(
        self,
        fused_lut: torch.Tensor,
        context: torch.Tensor,
        rgb: torch.Tensor,
        orig_h: int,
        orig_w: int,
        tile_size: int = 2048,
    ) -> torch.Tensor:
        """
        Apply 4D LUT in tiles to handle very large images without OOM.
        All tensors should already be on the same device.
        """
        B, _, N_ctx, D, _, _ = fused_lut.shape
        lut_0 = fused_lut[:, :, 0, :, :, :]  # [B, 3, D, D, D]
        lut_1 = fused_lut[:, :, 1, :, :, :]  # [B, 3, D, D, D]

        # If image fits in memory, process all at once
        if orig_h <= tile_size and orig_w <= tile_size:
            out_0 = trilinear_interpolate(lut_0, rgb)
            out_1 = trilinear_interpolate(lut_1, rgb)
            return out_0 * (1 - context) + out_1 * context

        # Process in tiles — compute on GPU, accumulate on CPU to save VRAM
        print(f"[SA-LUT] Processing in tiles ({tile_size}x{tile_size})...")
        result = torch.zeros_like(rgb, device="cpu")
        for y in range(0, orig_h, tile_size):
            for x in range(0, orig_w, tile_size):
                y_end = min(y + tile_size, orig_h)
                x_end = min(x + tile_size, orig_w)

                rgb_tile = rgb[:, :, y:y_end, x:x_end]
                ctx_tile = context[:, :, y:y_end, x:x_end]

                out_0 = trilinear_interpolate(lut_0, rgb_tile)
                out_1 = trilinear_interpolate(lut_1, rgb_tile)
                tile_result = out_0 * (1 - ctx_tile) + out_1 * ctx_tile

                result[:, :, y:y_end, x:x_end] = tile_result.cpu()

        return result

    def _resize_long_edge(self, img: np.ndarray, max_size: int) -> np.ndarray:
        h, w = img.shape[:2]
        if max(h, w) <= max_size:
            return img
        scale = max_size / max(h, w)
        new_h, new_w = int(h * scale), int(w * scale)
        return cv2.resize(img, (new_w, new_h), interpolation=cv2.INTER_AREA)
