"""
Pure-PyTorch replacements for CUDA quadrilinear/trilinear extensions.
"""
import torch
import torch.nn.functional as F


def trilinear_interpolate(lut, x):
    """
    Pure-PyTorch trilinear interpolation.

    Args:
        lut: [B, 3, D, D, D] or [3, D, D, D] – 3D LUT.
        x:   [B, 3, H, W] in [0, 1] – query RGB values.

    Returns:
        [B, 3, H, W] interpolated output.
    """
    if len(lut.shape) == 4:
        lut = lut.unsqueeze(0)
    B, C, D, _, _ = lut.shape
    # grid_sample 5D expects grid in [-1, 1]
    grid = x.permute(0, 2, 3, 1).unsqueeze(1)  # [B, 1, H, W, 3]
    grid = grid * 2.0 - 1.0
    sampled = F.grid_sample(
        lut, grid, mode="bilinear", padding_mode="border", align_corners=True
    )
    # sampled: [B, C, 1, H, W]
    return sampled.squeeze(2)


class TrilinearInterpolation(torch.nn.Module):
    def __init__(self):
        super().__init__()

    def forward(self, lut, x):
        if len(lut.shape) == 4:
            lut = lut.unsqueeze(0)
        if lut.shape[0] == x.shape[0]:
            res = torch.empty_like(x)
            for i in range(lut.shape[0]):
                res[i : i + 1] = trilinear_interpolate(lut[i : i + 1], x[i : i + 1])
            return res
        else:
            return trilinear_interpolate(lut[0:1], x)


class QuadrilinearInterpolation(torch.nn.Module):
    """
    Pure-PyTorch quadrilinear interpolation for 4D LUT.

    4D LUT shape: [B, 3, N_ctx, D, D, D]
    Query shape:  [B, 4, H, W] where channel 0 = context, 1-3 = RGB

    Since N_ctx is usually 2, we decompose into two trilinear lookups
    and linearly blend by the context channel.
    """

    def __init__(self):
        super().__init__()

    def forward(self, lut, x):
        if len(lut.shape) == 5:
            lut = lut.unsqueeze(0)
        B, C, N_ctx, D, _, _ = lut.shape
        assert N_ctx == 2, (
            f"QuadrilinearInterpolation only supports N_ctx==2, got {N_ctx}"
        )

        context = x[:, 0:1, :, :].clamp(0, 1)  # [B, 1, H, W]
        rgb = x[:, 1:4, :, :].clamp(0, 1)     # [B, 3, H, W]

        lut_0 = lut[:, :, 0, :, :, :]  # [B, 3, D, D, D]
        lut_1 = lut[:, :, 1, :, :, :]  # [B, 3, D, D, D]

        out_0 = trilinear_interpolate(lut_0, rgb)
        out_1 = trilinear_interpolate(lut_1, rgb)

        output = out_0 * (1 - context) + out_1 * context
        return lut, output
