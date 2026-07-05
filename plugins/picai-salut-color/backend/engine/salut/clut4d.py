"""4D LUT construction and TV regularisation (pure PyTorch)."""
import torch
import torch.nn as nn


def identity4d_tensor(dim, num_context_bins=2):
    step = torch.linspace(0, 1, steps=dim)
    LUT = torch.empty(3, dim, dim, dim)
    LUT[0] = step.unsqueeze(0).unsqueeze(0).expand(dim, dim, dim)
    LUT[1] = step.unsqueeze(-1).unsqueeze(0).expand(dim, dim, dim)
    LUT[2] = step.unsqueeze(-1).unsqueeze(-1).expand(dim, dim, dim)
    LUT = LUT.unsqueeze(1).expand(3, num_context_bins, dim, dim, dim).clone()
    return LUT


class CLUT4D(nn.Module):
    def __init__(self, num, dim=17, num_context_bins=2):
        super(CLUT4D, self).__init__()
        self.num = num
        self.dim = dim
        self.num_context_bins = num_context_bins
        self.LUTs = nn.Parameter(torch.zeros(num, 3, num_context_bins, dim, dim, dim))
        nn.init.uniform_(self.LUTs, -0.1, 0.1)

    def combine(self, weight, identity_lut):
        LUTs_flat = self.LUTs.view(self.num, -1)
        fused_flat = torch.matmul(weight, LUTs_flat)
        fused_lut = fused_flat.view(
            -1, 3, self.num_context_bins, self.dim, self.dim, self.dim
        )
        fused_lut = fused_lut + identity_lut.unsqueeze(0)
        fused_lut = torch.clamp(fused_lut, 0, 1)
        return fused_lut

    def forward(self, weight, identity_lut, tvmn_module=None):
        fused_lut = self.combine(weight, identity_lut)
        tvmn = 0
        if tvmn_module is not None:
            tvmn = tvmn_module(fused_lut)
        return fused_lut, tvmn


class TV_4D(nn.Module):
    def __init__(self, dim=17, num_context_bins=2):
        super(TV_4D, self).__init__()
        self.num_context_bins = num_context_bins
        weight_r = torch.ones(3, num_context_bins, dim, dim, dim - 1, dtype=torch.float)
        weight_r[:, :, :, :, (0, dim - 2)] *= 2.0
        weight_g = torch.ones(3, num_context_bins, dim, dim - 1, dim, dtype=torch.float)
        weight_g[:, :, :, (0, dim - 2), :] *= 2.0
        weight_b = torch.ones(3, num_context_bins, dim - 1, dim, dim, dtype=torch.float)
        weight_b[:, :, (0, dim - 2), :, :] *= 2.0
        self.register_buffer("weight_r", weight_r)
        self.register_buffer("weight_g", weight_g)
        self.register_buffer("weight_b", weight_b)
        if self.num_context_bins > 1:
            weight_c = torch.ones(3, num_context_bins - 1, dim, dim, dim, dtype=torch.float)
            if num_context_bins - 1 > 0:
                weight_c[:, (0, num_context_bins - 2), :, :, :] *= 2.0
            self.register_buffer("weight_c", weight_c)
        self.relu = torch.nn.ReLU()

    def forward(self, lut):
        dif_context = lut[:, :, :-1, :, :, :] - lut[:, :, 1:, :, :, :]
        dif_r = lut[:, :, :, :, :, :-1] - lut[:, :, :, :, :, 1:]
        dif_g = lut[:, :, :, :, :-1, :] - lut[:, :, :, :, 1:, :]
        dif_b = lut[:, :, :, :-1, :, :] - lut[:, :, :, 1:, :, :]
        tv = (
            torch.mean(torch.mul((dif_r ** 2), self.weight_r))
            + torch.mean(torch.mul((dif_g ** 2), self.weight_g))
            + torch.mean(torch.mul((dif_b ** 2), self.weight_b))
        )
        mn = (
            torch.mean(self.relu(dif_r))
            + torch.mean(self.relu(dif_g))
            + torch.mean(self.relu(dif_b))
            + torch.mean(self.relu(dif_context))
        )
        if self.num_context_bins > 1:
            tv += torch.mean(torch.mul((dif_context ** 2), self.weight_c))
            mn += torch.mean(self.relu(dif_context))
        return tv, mn
