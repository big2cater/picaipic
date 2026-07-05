import importlib
import os
import sys


def import_required(module_name: str):
    try:
        return importlib.import_module(module_name)
    except Exception as exc:
        print(f"Missing or broken dependency: {module_name}: {exc}", file=sys.stderr)
        raise


def module_version(module) -> str:
    return str(getattr(module, "__version__", "unknown"))


def main() -> int:
    backend = os.environ.get("PICAIPIC_PLUGIN_BACKEND", "").lower()
    modules = ["torch", "numpy", "cv2", "skimage", "timm", "yaml", "addict"]
    if backend == "directml":
        modules.append("torch_directml")

    loaded = {name: import_required(name) for name in modules}
    torch = loaded["torch"]

    print("Verified Python:", sys.executable)
    for name in modules:
        print(f"Verified import: {name} {module_version(loaded[name])}")
    print("Verified torch cuda available:", torch.cuda.is_available())
    print("Verified torch hip version:", getattr(getattr(torch, "version", object()), "hip", None))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
