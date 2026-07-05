#!/usr/bin/env python3
"""PicAiPic sandbox GPU spike v4 — Popen + kill to dodge ROCm exit deadlock.

Background
----------
ROCm 7.2 + torch on Windows: a subprocess that initializes a CUDA context
(via torch.cuda.is_available() or any GPU op) deadlocks in process exit —
amdhip64.dll's DLL_PROCESS_DETACH hangs. subprocess.run waits for exit,
so it times out (300s) even though the child finished its real work in <2s.

Fix: don't wait for exit. Spawn the child, have it write a "done" signal
file when the GPU work is finished, then terminate the child (TerminateProcess
bypasses the hung DLL cleanup). Verified: 1.8s end-to-end with GPU matmul.

This spike also exercises the deny-ACL sandbox: a temp dir is created with
icacls /deny <user>:(W), and the child runs under that restriction to confirm
deny-ACL on a directory does NOT block CUDA driver init (only file writes
into that dir).

Usage:
    python scripts/sandbox_gpu_spike.py
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

CHILD_SCRIPT = r"""
import sys, time, json, os
SIGNAL = sys.argv[1]
print("[child] start", flush=True)
t0 = time.perf_counter()
import torch
t1 = time.perf_counter()
print(f"[child] torch imported in {t1-t0:.1f}s", flush=True)
cuda = torch.cuda.is_available()
hip = getattr(getattr(torch, "version", object()), "hip", None)
print(f"[child] cuda={cuda} hip={hip}", flush=True)
if cuda:
    a = torch.randn(256, 256, device="cuda")
    b = torch.randn(256, 256, device="cuda")
    c = a @ b
    torch.cuda.synchronize()  # ensure work completes before signaling
    print(f"[child] gpu matmul ok shape={c.shape}", flush=True)
else:
    a = torch.randn(256, 256)
    b = torch.randn(256, 256)
    c = a @ b
    print(f"[child] cpu matmul ok shape={c.shape}", flush=True)
# Signal parent that real work is done. Do NOT rely on process exit —
# ROCm DLL cleanup may hang on this process.
with open(SIGNAL, "w", encoding="utf-8") as f:
    json.dump({"ok": True, "import_s": round(t1 - t0, 1),
               "cuda": cuda, "hip": hip}, f)
    f.flush()
    os.fsync(f.fileno())
print("[child] signal written, returning (cleanup may hang — parent will kill)", flush=True)
"""


def _win_username() -> str:
    return os.environ.get("USERNAME") or os.environ.get("USER") or ""


def _make_denied_dir() -> tuple[Path, str] | None:
    """Create a temp dir and deny write for current user via icacls.

    Returns (dir, username) or None if icacls unavailable / failed.
    The deny-ACL only blocks writes INTO this dir; it must not affect
    CUDA driver init (which loads from ROCm install + torch lib dirs).
    """
    user = _win_username()
    if not user:
        return None
    d = Path(tempfile.mkdtemp(prefix="picaipic_acl_"))
    try:
        r = subprocess.run(
            ["icacls", str(d), "/deny", f"{user}:(W)"],
            capture_output=True, text=True, timeout=15,
        )
        if r.returncode != 0:
            print(f"  icacls deny failed (rc={r.returncode}): {r.stderr.strip()}")
            shutil.rmtree(d, ignore_errors=True)
            return None
        return d, user
    except (FileNotFoundError, subprocess.TimeoutExpired) as exc:
        print(f"  icacls not available: {exc}")
        shutil.rmtree(d, ignore_errors=True)
        return None


def _restore_and_cleanup(d: Path, user: str) -> None:
    try:
        subprocess.run(
            ["icacls", str(d), "/remove:d", f"{user}"],
            capture_output=True, text=True, timeout=15,
        )
    finally:
        shutil.rmtree(d, ignore_errors=True)


def _run_child(python_exe: str, child_script: Path, signal_file: Path,
               env: dict, workdir: str | None, label: str, timeout: float) -> dict | None:
    """Spawn child, wait for done-signal file, then kill (don't wait for exit).

    Returns parsed signal dict on success, None on timeout.
    """
    print(f"--- {label} ---")
    if signal_file.exists():
        signal_file.unlink()
    t0 = time.perf_counter()
    proc = subprocess.Popen(
        [python_exe, str(child_script), str(signal_file)],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        env=env, cwd=workdir,
    )
    deadline = time.perf_counter() + timeout
    while time.perf_counter() < deadline:
        if signal_file.exists():
            break
        # also detect early crash
        if proc.poll() is not None:
            break
        time.sleep(0.1)

    elapsed = time.perf_counter() - t0
    if not signal_file.exists():
        print(f"  TIMEOUT after {elapsed:.1f}s (no done signal)")
        proc.kill()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            pass
        out = proc.stdout.read() if proc.stdout else ""
        err = proc.stderr.read() if proc.stderr else ""
        if out:
            print(f"  stdout: {out[:300]}")
        if err:
            print(f"  stderr: {err[:300]}")
        return None

    # Signal received — real work is done. Kill to dodge ROCm exit hang.
    result = json.loads(signal_file.read_text(encoding="utf-8"))
    print(f"  done signal in {elapsed:.1f}s: {result}")
    # Give stdout a moment to flush, then terminate.
    try:
        proc.terminate()
        proc.wait(timeout=5)
        print(f"  child exited rc={proc.returncode}")
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
        print("  child killed (terminate hung — expected ROCm cleanup deadlock)")
    return result


def main() -> int:
    if sys.platform != "win32":
        print("Windows-only.", file=sys.stderr)
        return 1

    python_exe = sys.executable
    print("=== PicAiPic Sandbox GPU Spike v4 ===")
    print(f"Python: {python_exe}")
    print(f"PICAIPIC_PLUGIN_AUTH_TOKEN set: {bool(os.environ.get('PICAIPIC_PLUGIN_AUTH_TOKEN'))}")
    print()
    print("NOTE: child is killed after signaling done, to dodge ROCm 7.2")
    print("      DLL_PROCESS_DETACH deadlock on process exit.")
    print()

    child_script = Path(tempfile.gettempdir()) / "picaipic_spike_child_v4.py"
    child_script.write_text(CHILD_SCRIPT, encoding="utf-8")
    signal_file = Path(tempfile.gettempdir()) / "picaipic_spike_signal_v4.json"

    env = os.environ.copy()
    print(f"PATH length: {len(env.get('PATH', ''))} chars")
    print(f"HSA_ENABLE_SDMA set: {bool(env.get('HSA_ENABLE_SDMA'))}")
    print(f"PYTORCH_HIP_ALLOC_CONF set: {bool(env.get('PYTORCH_HIP_ALLOC_CONF'))}")
    print()

    # Pass 0: in-process baseline (parent does not exit, no cleanup hang).
    print("=== Pass 0: in-process (baseline, no subprocess) ===")
    t0 = time.perf_counter()
    try:
        import importlib
        importlib.import_module("torch")
        print(f"in-process torch import: {time.perf_counter() - t0:.1f}s")
    except Exception as exc:
        print(f"in-process torch import failed: {exc}")
    print()

    # Pass 1: subprocess, no ACL — confirms Popen+kill works around exit hang.
    print("=== Pass 1: subprocess, no ACL ===")
    r1 = _run_child(python_exe, child_script, signal_file, env,
                    workdir=None, label="plain subprocess", timeout=60)
    print()

    # Pass 2: subprocess with deny-ACL on workdir — confirms deny-ACL on a dir
    # does NOT block CUDA driver init (only writes into that dir).
    print("=== Pass 2: subprocess, deny-ACL on workdir ===")
    acl = _make_denied_dir()
    if acl is None:
        print("  (skipped: icacls unavailable)")
        r2 = None
    else:
        denied_dir, user = acl
        print(f"  denied dir: {denied_dir} (user={user}, deny W)")
        r2 = _run_child(python_exe, child_script, signal_file, env,
                        workdir=str(denied_dir), label="deny-ACL workdir",
                        timeout=60)
        _restore_and_cleanup(denied_dir, user)
    print()

    # Summary
    print("=== Summary ===")
    print(f"Pass 1 (subprocess, no ACL): {'OK' if r1 and r1.get('ok') else 'FAIL'}")
    if r1:
        print(f"  cuda={r1.get('cuda')} import={r1.get('import_s')}s")
    if acl is not None:
        print(f"Pass 2 (deny-ACL workdir):  {'OK' if r2 and r2.get('ok') else 'FAIL'}")
        if r2:
            print(f"  cuda={r2.get('cuda')} import={r2.get('import_s')}s")
        print("  => deny-ACL on a dir does NOT block CUDA init (confirmed"
              if r2 and r2.get('cuda')
              else "  => check result", ")")
    else:
        print("Pass 2 (deny-ACL workdir):  skipped")

    child_script.unlink(missing_ok=True)
    signal_file.unlink(missing_ok=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
