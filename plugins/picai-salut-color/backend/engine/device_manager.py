"""
Unified device manager for cross-vendor GPU detection.
Supports NVIDIA CUDA, AMD ROCm (HIP), Apple MPS, Intel XPU,
Microsoft DirectML, and CPU fallback.

Priority for AMD RX 7000+ cards:
  1. ROCm (HIP)   -- best performance, native PyTorch
  2. DirectML     -- Windows fallback if ROCm not installed
  3. CPU
"""

import re
import subprocess
import sys
from typing import Any, Dict, List, Optional, Union

import torch


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

def resolve_device(preference: str = "auto") -> Union[str, torch.device]:
    """
    Resolve a human-readable device preference into a torch device string
    or a DirectML torch.device object.

    Supported preferences:
        "auto"     -> auto-detect best available backend
        "cuda"     -> NVIDIA CUDA / AMD ROCm (if installed)
        "mps"      -> Apple Silicon Metal Performance Shaders
        "xpu"      -> Intel XPU (Arc / Data Center)
        "directml" -> Microsoft DirectML (Windows, cross-vendor)
        "cpu"      -> CPU fallback
    """
    preference = (preference or "auto").lower().strip()

    if preference == "auto":
        return _auto_detect()
    if preference == "cuda":
        return _try_cuda()
    if preference == "mps":
        return _try_mps()
    if preference == "xpu":
        return _try_xpu()
    if preference == "directml":
        return _try_directml()
    if preference == "cpu":
        return "cpu"

    # Allow direct torch device strings, e.g. "cuda:0"
    return preference


def get_device_info(device: Union[str, torch.device]) -> dict:
    """
    Return a dict with human-readable device metadata for logging / API.
    """
    info = {
        "device": str(device),
        "backend": "unknown",
        "name": "Unknown",
        "memory_mb": None,
    }

    dev_str = str(device)

    if _is_directml_device(device):
        info["backend"] = "directml"
        info["device"] = "directml"
        info["name"] = "DirectML (Generic)"
        try:
            import torch_directml
            device_index = getattr(device, "index", 0)
            info["name"] = f"DirectML: {torch_directml.device_name(device_index)}"
        except Exception:
            pass

        gpu_info = _match_gpu_info_for_device(info["name"])
        if gpu_info:
            info["memory_mb"] = gpu_info.get("memory_mb")
            if info["name"] == "DirectML (Generic)":
                info["name"] = gpu_info["name"]
        return info

    if dev_str.startswith("cuda"):
        info["backend"] = "rocm" if _is_rocm() else "cuda"
        try:
            idx = int(dev_str.split(":")[1]) if ":" in dev_str else 0
            info["name"] = torch.cuda.get_device_name(idx)
            info["memory_mb"] = torch.cuda.get_device_properties(idx).total_memory // (1024 * 1024)
        except Exception:
            pass
        return info

    if dev_str == "mps":
        info["backend"] = "mps"
        info["name"] = "Apple Metal Performance Shaders"
        return info

    if dev_str.startswith("xpu"):
        info["backend"] = "xpu"
        try:
            idx = int(dev_str.split(":")[1]) if ":" in dev_str else 0
            info["name"] = torch.xpu.get_device_name(idx)
            info["memory_mb"] = torch.xpu.get_device_properties(idx).total_memory // (1024 * 1024)
        except Exception:
            pass
        return info

    if "directml" in dev_str.lower():
        info["backend"] = "directml"
        info["name"] = "DirectML (Generic)"
        try:
            import torch_directml
            device_index = getattr(device, "index", 0)
            info["name"] = f"DirectML: {torch_directml.device_name(device_index)}"
        except Exception:
            pass
        gpu_info = _match_gpu_info_for_device(info["name"])
        if gpu_info:
            info["memory_mb"] = gpu_info.get("memory_mb")
            if info["name"] == "DirectML (Generic)":
                info["name"] = gpu_info["name"]
        return info

    if dev_str == "cpu":
        info["backend"] = "cpu"
        info["name"] = "CPU"
        return info

    return info


def list_available_backends() -> list:
    """Return a list of available backend names on this machine."""
    backends = []
    if torch.cuda.is_available():
        backends.append("cuda")
    if torch.backends.mps.is_available():
        backends.append("mps")
    if _has_xpu() and torch.xpu.is_available():
        backends.append("xpu")
    if _has_directml():
        backends.append("directml")
    backends.append("cpu")
    return backends


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------

def _auto_detect() -> Union[str, torch.device]:
    try:
        # --- 1. AMD RX 7000+ priority: try ROCm first --------------------
        if _is_amd_rx_7000_or_above():
            if _is_rocm() and torch.cuda.is_available():
                print("[DeviceManager] AMD RX 7000+ detected, using ROCm (HIP)")
                return "cuda"

            print("[DeviceManager] AMD RX 7000+ detected but ROCm PyTorch not available.")
            print("[DeviceManager]   Install a ROCm-capable PyTorch runtime, or use DirectML/CPU fallback.")
            print("[DeviceManager] Falling back to DirectML / CPU.")

        # --- 2. NVIDIA / AMD ROCm (both expose torch.cuda) ---------------
        if torch.cuda.is_available():
            return _try_cuda()

        # --- 3. Apple Silicon --------------------------------------------
        if torch.backends.mps.is_available():
            return _try_mps()

        # --- 4. Intel XPU (Arc, Data Center) -----------------------------
        if _has_xpu() and torch.xpu.is_available():
            return _try_xpu()

        # --- 5. DirectML (Windows cross-vendor fallback) -----------------
        if _has_directml():
            dml = _try_directml()
            if dml is not None:
                return dml

        return "cpu"
    except Exception as e:
        print(f"[DeviceManager] Auto-detection crashed: {e}")
        print("[DeviceManager] Falling back to CPU.")
        return "cpu"


def _try_cuda() -> str:
    if torch.cuda.is_available():
        return "cuda"
    print("[DeviceManager] CUDA/ROCm requested but not available, falling back to CPU.")
    return "cpu"


def _try_mps() -> str:
    if torch.backends.mps.is_available():
        return "mps"
    print("[DeviceManager] MPS requested but not available, falling back to CPU.")
    return "cpu"


def _try_xpu() -> str:
    if _has_xpu() and torch.xpu.is_available():
        return "xpu"
    print("[DeviceManager] XPU requested but not available, falling back to CPU.")
    return "cpu"


def _try_directml() -> Union[torch.device, None]:
    try:
        import torch_directml
        dml_device = torch_directml.device()
        print(f"[DeviceManager] DirectML device ready: {dml_device}")
        return dml_device
    except ImportError:
        return None
    except Exception as e:
        print(f"[DeviceManager] DirectML init failed: {e}")
        return None


def _is_rocm() -> bool:
    """Return True if the current torch is built against ROCm/HIP."""
    return hasattr(torch.version, "hip") and torch.version.hip is not None


def _has_xpu() -> bool:
    return hasattr(torch, "xpu")


def _has_directml() -> bool:
    try:
        import importlib.util
        return importlib.util.find_spec("torch_directml") is not None
    except Exception:
        return False


def _is_directml_device(device: Union[str, torch.device]) -> bool:
    try:
        if isinstance(device, torch.device) and device.type == "privateuseone":
            return True
    except Exception:
        pass

    dev_str = str(device).lower()
    return "directml" in dev_str or dev_str.startswith("privateuseone")


# ---------------------------------------------------------------------------
# GPU model detection (OS-level)
# ---------------------------------------------------------------------------

def _get_system_gpu_info() -> List[Dict[str, Optional[int]]]:
    """Query system GPU metadata via OS-native tools."""
    gpus: List[Dict[str, Optional[int]]] = []
    if sys.platform == "win32":
        try:
            result = subprocess.run(
                [
                    "powershell",
                    "-NoProfile",
                    "-Command",
                    (
                        "Get-CimInstance Win32_VideoController | "
                        "Select-Object Name,AdapterRAM | "
                        "ForEach-Object { "
                        "  $ram = if ($_.AdapterRAM) { [math]::Round($_.AdapterRAM / 1MB) } else { '' }; "
                        "  Write-Output ($_.Name + '||' + $ram) "
                        "}"
                    ),
                ],
                capture_output=True, text=True, timeout=10,
            )
            for line in result.stdout.splitlines():
                line = line.strip()
                if line:
                    parts = line.split("||", 1)
                    name = parts[0].strip()
                    memory_mb = None
                    if len(parts) > 1 and parts[1].strip():
                        try:
                            memory_mb = int(float(parts[1].strip()))
                        except Exception:
                            memory_mb = None
                    gpus.append({"name": name, "memory_mb": memory_mb})
        except Exception:
            pass

        if not gpus:
            try:
                result = subprocess.run(
                    ["wmic", "path", "win32_VideoController", "get", "Name"],
                    capture_output=True, text=True, timeout=5,
                )
                for line in result.stdout.splitlines()[1:]:
                    line = line.strip()
                    if line:
                        gpus.append({"name": line, "memory_mb": None})
            except Exception:
                pass
    else:
        try:
            result = subprocess.run(
                ["lspci"], capture_output=True, text=True, timeout=5,
            )
            for line in result.stdout.splitlines():
                if "VGA" in line or "3D controller" in line or "Display controller" in line:
                    parts = line.split(":")
                    if len(parts) >= 3:
                        gpus.append({"name": parts[2].strip(), "memory_mb": None})
        except Exception:
            pass
    return gpus


def _get_gpu_names() -> list:
    """Query system GPU names via OS-native tools."""
    return [gpu["name"] for gpu in _get_system_gpu_info() if gpu.get("name")]


def _match_gpu_info_for_device(device_name: str) -> Optional[Dict[str, Optional[int]]]:
    candidates = _get_system_gpu_info()
    if not candidates:
        return None

    normalized = device_name.upper().replace("DIRECTML:", "").strip()
    for gpu in candidates:
        gpu_name = (gpu.get("name") or "").upper()
        if not gpu_name:
            continue
        if gpu_name in normalized or normalized in gpu_name:
            return gpu

    for gpu in candidates:
        gpu_name = (gpu.get("name") or "").upper()
        if "MICROSOFT BASIC DISPLAY" in gpu_name:
            continue
        return gpu

    return None


def _is_amd_rx_7000_or_above() -> bool:
    """
    Detect whether any installed GPU is an AMD Radeon RX 7000 series
    or newer (RDNA 3 / RDNA 4), which have official ROCm support.
    """
    names = _get_gpu_names()
    for name in names:
        n = name.upper()
        if "AMD" not in n and "RADEON" not in n:
            continue
        # RX 7000 series: 7xxx, 7600, 7700, 7800, 7900
        # RX 9000 series: 9xxx, 9060, 9070, 9090
        if re.search(r"RX\s*(7\d{3}|9\d{3}|9060|9070|9090)", n):
            return True
        # Radeon PRO W7000 / W9000 workstation cards (RDNA 3)
        if re.search(r"PRO\s*W\s*(7\d{3}|9\d{3})", n):
            return True
    return False


# ---------------------------------------------------------------------------
# Convenience: print banner at startup
# ---------------------------------------------------------------------------

def print_device_banner(device: Union[str, torch.device]) -> None:
    info = get_device_info(device)
    mem = f"{info['memory_mb']} MB" if info["memory_mb"] else "N/A"
    print("=" * 50)
    print(f"  Device : {info['name']}")
    print(f"  Backend: {info['backend'].upper()}")
    print(f"  Memory : {mem}")
    print("=" * 50)
