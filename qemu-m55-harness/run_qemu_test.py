#!/usr/bin/env python3
"""Cortex-M55 (STM32N6-class) runner for the QCEOM-RL no_std kernel tests.

Boots the ELF on QEMU's `mps3-an547` board (real ARMv8.1-M Cortex-M55),
pipes the semihosting output to the terminal, and exits 0/1 to match the
harness's semihosting `debug::exit` code â€” so `cargo run` behaves like
`cargo test` on emulated silicon.

Adapted from LUXERON/NO_STD-QEMU-TEST-HARNESS (cortex-m55 path), with one
addition: on Windows the script shells into WSL for qemu-system-arm and
translates the ELF path to /mnt/<drive>/...

Requires: qemu-system-arm >= 8.2 (native, or inside WSL on Windows).
Invoked automatically by `cargo run` via .cargo/config.toml.
"""
import os
import re
import subprocess
import sys
import threading

TIMEOUT_S = 900.0  # engine training on TCG soft-float f64 takes a while


def to_wsl_path(win_path: str) -> str:
    p = os.path.abspath(win_path)
    m = re.match(r"^([A-Za-z]):[\\/](.*)$", p)
    if not m:
        return p
    drive, rest = m.group(1).lower(), m.group(2).replace("\\", "/")
    return f"/mnt/{drive}/{rest}"


def main():
    if len(sys.argv) < 2:
        print("Usage: run_qemu_test.py <kernel_elf>")
        sys.exit(1)

    kernel = sys.argv[1]
    qemu_args = [
        "qemu-system-arm",
        "-machine", "mps3-an547",
        "-cpu", "cortex-m55",
        "-nographic",
        "-semihosting-config", "enable=on,target=native",
        "-kernel", to_wsl_path(kernel) if os.name == "nt" else kernel,
    ]
    cmd = (["wsl", "--"] + qemu_args) if os.name == "nt" else qemu_args
    print("Running QEMU:", " ".join(cmd))

    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)

    def kill_proc():
        print("\n--- TIMEOUT EXCEEDED ---")
        p.kill()

    timer = threading.Timer(TIMEOUT_S, kill_proc)
    timer.start()
    try:
        for line in p.stdout:
            sys.stdout.write(line)
    finally:
        timer.cancel()

    rc = p.wait()
    if rc == 0:
        print("--- TEST PASSED ---")
        sys.exit(0)
    print(f"--- TEST FAILED (qemu rc={rc}) ---")
    sys.exit(1)


if __name__ == "__main__":
    main()
