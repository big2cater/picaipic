"""
SA-LUT main model (pure-PyTorch port).
Removes CUDA extension dependencies; uses PyTorch grid_sample instead.
"""
import os
from pathlib import Path

import torch
import torch.nn as nn
import torch.nn.functional as F

from . import net as net_module
from .clut4d import CLUT4D, TV_4D, identity4d_tensor
from .pytorch_interpolation import TrilinearInterpolation, QuadrilinearInterpolation


# ------------------------------------------------------------------
# Helpers
# ------------------------------------------------------------------
def _load_vgg_weights(vgg_model, weight_path: str):
    """Load VGG normalised weights; fallback to torchvision VGG19 if missing."""
    p = Path(weight_path)
    if p.exists():
        vgg_model.load_state_dict(torch.load(str(p), weights_only=False))
        return
    try:
        import torchvision.models as models
        vgg19 = models.vgg19(weights=models.VGG19_Weights.IMAGENET1K_V1).features
        # Heuristic mapping: our custom vgg has same layer order as torchvision.features
        state = vgg19.state_dict()
        mapped = {}
        for k, v in state.items():
            mapped[k] = v
        vgg_model.load_state_dict(mapped, strict=False)
        print("[SA-LUT] VGG normalised weights not found; using torchvision VGG19 fallback.")
    except Exception as e:
        print(f"[SA-LUT] Warning: could not load any VGG weights: {e}")


def _make_identity_3dlut(dim=33):
    """Create a simple identity 3D LUT tensor [3, dim, dim, dim]."""
    step = torch.linspace(0, 1, steps=dim)
    lut = torch.empty(3, dim, dim, dim)
    lut[0] = step.view(1, 1, -1).expand(dim, dim, dim)
    lut[1] = step.view(1, -1, 1).expand(dim, dim, dim)
    lut[2] = step.view(-1, 1, 1).expand(dim, dim, dim)
    return lut


def adaptive_instance_normalization(content_feat, style_feat, eps=1e-5):
    B, C, H, W = content_feat.size()
    content_mean = content_feat.view(B, C, -1).mean(dim=2).view(B, C, 1, 1)
    content_std = content_feat.view(B, C, -1).std(dim=2).view(B, C, 1, 1) + eps
    style_mean = style_feat.view(B, C, -1).mean(dim=2).view(B, C, 1, 1)
    style_std = style_feat.view(B, C, -1).std(dim=2).view(B, C, 1, 1)
    normalized = (content_feat - content_mean) / content_std
    return normalized * style_std + style_mean


# ------------------------------------------------------------------
# Building blocks
# ------------------------------------------------------------------
class ConvLayer(nn.Module):
    def __init__(self, in_channels, out_channels, kernel_size, stride):
        super().__init__()
        reflection_padding = kernel_size // 2
        self.reflection_pad = nn.ReflectionPad2d(reflection_padding)
        self.conv2d = nn.Conv2d(in_channels, out_channels, kernel_size, stride)
        nn.init.normal_(self.conv2d.weight, mean=0, std=0.5)

    def forward(self, x):
        out = self.reflection_pad(x)
        out = self.conv2d(out)
        return out


class SplattingBlock2(nn.Module):
    def __init__(self, in_channels, out_channels):
        super().__init__()
        self.conv1 = ConvLayer(in_channels, in_channels, 3, 1)
        self.conv2 = ConvLayer(in_channels, out_channels, 3, 1)

    def forward(self, s):
        s1 = torch.tanh(self.conv1(s))
        s = torch.tanh(self.conv2(s1 + s))
        return s


class ResidualBlock(nn.Module):
    def __init__(self, channels):
        super().__init__()
        self.conv1 = nn.Conv2d(channels, channels, kernel_size=3, padding=1)
        self.in1 = nn.InstanceNorm2d(channels)
        self.conv2 = nn.Conv2d(channels, channels, kernel_size=3, padding=1)
        self.in2 = nn.InstanceNorm2d(channels)
        self.relu = nn.LeakyReLU(0.2, inplace=True)

    def forward(self, x):
        residual = x
        out = self.relu(self.in1(self.conv1(x)))
        out = self.in2(self.conv2(out))
        out = out + residual
        out = self.relu(out)
        return out


class ChannelAttention(nn.Module):
    def __init__(self, channels, reduction=8):
        super().__init__()
        self.avg_pool = nn.AdaptiveAvgPool2d(1)
        self.max_pool = nn.AdaptiveMaxPool2d(1)
        self.fc = nn.Sequential(
            nn.Conv2d(channels, channels // reduction, 1, bias=False),
            nn.ReLU(inplace=True),
            nn.Conv2d(channels // reduction, channels, 1, bias=False),
        )
        self.sigmoid = nn.Sigmoid()

    def forward(self, x):
        avg_out = self.fc(self.avg_pool(x))
        max_out = self.fc(self.max_pool(x))
        out = avg_out + max_out
        scale = self.sigmoid(out)
        return x * scale


class CrossAttentionContextGenerator(nn.Module):
    def __init__(self, in_channels=3, base_channels=32, attn_channels=64, target_resolution=256):
        super().__init__()
        self.target_resolution = target_resolution

        self.content_encoder = nn.Sequential(
            nn.Conv2d(in_channels, base_channels, kernel_size=3, stride=1, padding=1),
            nn.InstanceNorm2d(base_channels),
            nn.LeakyReLU(0.2, inplace=True),
            ResidualBlock(base_channels),
            nn.Conv2d(base_channels, base_channels, kernel_size=3, stride=2, padding=1),
            nn.InstanceNorm2d(base_channels),
            nn.LeakyReLU(0.2, inplace=True),
        )
        self.style_encoder = nn.Sequential(
            nn.Conv2d(in_channels, base_channels, kernel_size=3, stride=1, padding=1),
            nn.InstanceNorm2d(base_channels),
            nn.LeakyReLU(0.2, inplace=True),
            ResidualBlock(base_channels),
            nn.Conv2d(base_channels, base_channels, kernel_size=3, stride=2, padding=1),
            nn.InstanceNorm2d(base_channels),
            nn.LeakyReLU(0.2, inplace=True),
        )
        self.proj_query = nn.Sequential(
            nn.Conv2d(base_channels, attn_channels, kernel_size=1),
            nn.InstanceNorm2d(attn_channels),
            nn.LeakyReLU(0.2, inplace=True),
        )
        self.proj_key = nn.Sequential(
            nn.Conv2d(base_channels, attn_channels, kernel_size=1),
            nn.InstanceNorm2d(attn_channels),
            nn.LeakyReLU(0.2, inplace=True),
        )
        self.proj_value = nn.Sequential(
            nn.Conv2d(base_channels, attn_channels, kernel_size=1),
            nn.InstanceNorm2d(attn_channels),
            nn.LeakyReLU(0.2, inplace=True),
        )
        self.attn_temperature = nn.Parameter(torch.tensor(1.0))
        self.channel_attention = ChannelAttention(attn_channels)
        self.modulation_conv = nn.Sequential(
            nn.Conv2d(base_channels + attn_channels, base_channels, kernel_size=3, padding=1),
            nn.InstanceNorm2d(base_channels),
            nn.LeakyReLU(0.2, inplace=True),
            ResidualBlock(base_channels),
            nn.Conv2d(base_channels, base_channels, kernel_size=3, padding=1),
            nn.InstanceNorm2d(base_channels),
            nn.LeakyReLU(0.2, inplace=True),
        )
        self.fixed_conv = nn.Sequential(
            nn.Conv2d(base_channels, base_channels, kernel_size=3, stride=1, padding=1),
            nn.InstanceNorm2d(base_channels),
            nn.LeakyReLU(0.2, inplace=True),
        )
        self.out_conv = nn.Sequential(
            nn.Conv2d(base_channels, base_channels // 2, kernel_size=3, padding=1),
            nn.InstanceNorm2d(base_channels // 2),
            nn.LeakyReLU(0.2, inplace=True),
            nn.Conv2d(base_channels // 2, 1, kernel_size=3, padding=1),
            nn.Sigmoid(),
        )
        self.upsample = nn.Upsample(scale_factor=2, mode="bilinear", align_corners=True)
        self._init_weights()

    def _init_weights(self):
        for m in self.modules():
            if isinstance(m, nn.Conv2d):
                nn.init.kaiming_normal_(m.weight, mode="fan_out", nonlinearity="leaky_relu")
                if m.bias is not None:
                    nn.init.constant_(m.bias, 0)
            elif isinstance(m, nn.InstanceNorm2d):
                if m.weight is not None:
                    nn.init.constant_(m.weight, 1)
                if m.bias is not None:
                    nn.init.constant_(m.bias, 0)

    def forward(self, content, style):
        orig_size = content.shape[-2:]
        if isinstance(self.target_resolution, int):
            target = (self.target_resolution, self.target_resolution)
        else:
            target = self.target_resolution
        if target is not None and orig_size != target:
            content_small = F.interpolate(content, size=target, mode="bilinear", align_corners=True)
            style_small = F.interpolate(style, size=target, mode="bilinear", align_corners=True)
        else:
            content_small = content
            style_small = style

        feat_content = self.content_encoder(content_small)
        feat_style = self.style_encoder(style_small)

        Q = self.proj_query(feat_content)
        K = self.proj_key(feat_style)
        V = self.proj_value(feat_style)

        B, C_attn, H_small, W_small = Q.shape
        num_tokens = H_small * W_small
        Q_flat = Q.view(B, C_attn, num_tokens).permute(0, 2, 1)
        K_flat = K.view(B, C_attn, num_tokens)
        V_flat = V.view(B, C_attn, num_tokens).permute(0, 2, 1)

        attn_scores = torch.bmm(Q_flat, K_flat)
        attn_scores = attn_scores / (C_attn ** 0.5) * self.attn_temperature
        attn_weights = F.softmax(attn_scores, dim=-1)

        aggregated = torch.bmm(attn_weights, V_flat)
        aggregated = aggregated.permute(0, 2, 1).view(B, C_attn, H_small, W_small)
        aggregated = self.channel_attention(aggregated)

        aggregated_full = self.upsample(aggregated)
        feat_content_full = self.upsample(feat_content)

        combined = torch.cat([feat_content_full, aggregated_full], dim=1)
        modulation = self.modulation_conv(combined)
        conv_out = self.fixed_conv(feat_content_full)
        dynamic_out = conv_out * modulation + feat_content_full
        context_map = self.out_conv(dynamic_out)

        if target is not None and orig_size != target:
            context_map = F.interpolate(context_map, size=orig_size, mode="bilinear", align_corners=True)
        return context_map


# ------------------------------------------------------------------
# Main Model
# ------------------------------------------------------------------
class VLog2StyleNet4D(nn.Module):
    def __init__(self, dim=17, num_basis=64, vgg_weight_path: str = "", standard_lut_path: str = ""):
        super().__init__()
        self.dim = dim
        self.num_basis = num_basis

        # VGG encoder
        vgg = net_module.vgg
        if vgg_weight_path:
            _load_vgg_weights(vgg, vgg_weight_path)
        self.encoder = net_module.Net(vgg)

        # Standard LUT (fallback to identity if missing)
        if standard_lut_path and Path(standard_lut_path).exists():
            standard_lut = torch.load(standard_lut_path, weights_only=False)
            if not isinstance(standard_lut, torch.Tensor):
                standard_lut = _make_identity_3dlut(dim=33)
        else:
            standard_lut = _make_identity_3dlut(dim=33)
        self.register_buffer("standard_lut", standard_lut, persistent=False)

        # Splatting blocks
        self.SB2 = SplattingBlock2(64, 256)
        self.SB3 = SplattingBlock2(128, 256)
        self.SB4 = SplattingBlock2(256, 256)
        self.SB5 = SplattingBlock2(512, 256)
        self.pg2 = nn.AdaptiveAvgPool2d(3)
        self.pg3 = nn.AdaptiveAvgPool2d(3)
        self.pg4 = nn.AdaptiveAvgPool2d(3)
        self.pg5 = nn.AdaptiveAvgPool2d(3)

        self.context_extractor = CrossAttentionContextGenerator(target_resolution=(256, 256))

        last_channel = 256 * 4
        self.classifier = nn.Sequential(
            nn.Conv2d(last_channel, 512, 3, 2),
            nn.Tanh(),
            nn.Conv2d(512, 512 * 2, 1, 1),
            nn.Tanh(),
            nn.Conv2d(512 * 2, 512, 1, 1),
            nn.Tanh(),
            nn.Conv2d(512, num_basis, 1, 1),
        )

        id_lut = identity4d_tensor(dim)
        self.register_buffer("id_lut", id_lut)
        self.CLUTs = CLUT4D(num=num_basis, dim=dim)
        self.tvmn = TV_4D(dim=dim)

        self.quadrilinear_interpolation = QuadrilinearInterpolation()
        self.trilinear_interpolation = TrilinearInterpolation()

    def forward(self, style, content):
        # Feature extraction
        style_feats = self.encoder.encode_with_intermediate(style)
        content_feats = self.encoder.encode_with_intermediate(content)

        # AdaIN fusion
        fused2 = adaptive_instance_normalization(content_feats[-4], style_feats[-4])
        fused3 = adaptive_instance_normalization(content_feats[-3], style_feats[-3])
        fused4 = adaptive_instance_normalization(content_feats[-2], style_feats[-2])
        fused5 = adaptive_instance_normalization(content_feats[-1], style_feats[-1])

        fused2 = self.SB2(fused2)
        fused3 = self.SB3(fused3)
        fused4 = self.SB4(fused4)
        fused5 = self.SB5(fused5)

        # Weight prediction
        pooled2 = self.pg2(fused2)
        pooled3 = self.pg3(fused3)
        pooled4 = self.pg4(fused4)
        pooled5 = self.pg5(fused5)
        combined_feature = torch.cat([pooled2, pooled3, pooled4, pooled5], dim=1)
        weight = self.classifier(combined_feature)[:, :, 0, 0]
        weight = torch.softmax(weight, dim=1)

        # Optional standard LUT application
        content_lut_applied = self.trilinear_interpolation(self.standard_lut.unsqueeze(0), content)
        context_map = self.context_extractor(content, style).clamp(0, 1)

        # LUT reconstruction
        lut_res, _ = self.CLUTs(weight, self.id_lut, self.tvmn)
        fused_lut = torch.clamp(lut_res, min=0, max=1)

        # Apply 4D LUT
        combined_input = torch.cat([context_map, content], dim=1)
        output = torch.zeros_like(content)
        for b in range(content.size(0)):
            _, out_b = self.quadrilinear_interpolation(fused_lut[b : b + 1], combined_input[b : b + 1])
            output[b : b + 1] = out_b

        return output, fused_lut, context_map
