"""Wait for limits before importing an engine; Local Studio remains lifecycle owner."""
import os
from pathlib import Path
import sys
import time

LIMIT = 36 * 1024**3
HEADROOM = 38 * 1024**3
EXPECTED = ["/models", "--served-model-name", "avesra-owned-27b", "--host", "0.0.0.0",
            "--port", "8000", "--max-model-len", "4096", "--gpu-memory-utilization", "0.25",
            "--max-num-seqs", "1", "--reasoning-parser", "qwen3", "--max-num-batched-tokens", "512",
            "--kv-cache-memory-bytes", "805306368", "--enforce-eager", "--language-model-only"]


def read(path):
    with open(path, "rb") as source:
        value = source.read(8193)
    if len(value) > 8192:
        raise ValueError("bounded_resource_metadata")
    return value.decode().strip()


def ready():
    root = Path("/sys/fs/cgroup")
    memory = read(root / "memory.max")
    quota, period = read(root / "cpu.max").split()
    swap = read(root / "memory.swap.max")
    if memory == "max" or quota == "max" or swap != "0":
        return False
    if not 0 < int(memory) <= LIMIT or not 0 < int(quota) <= 2 * int(period):
        return False
    values = dict(line.split(":", 1) for line in read("/proc/meminfo").splitlines())
    available = int(values["MemAvailable"].split()[0]) * 1024
    return available >= HEADROOM


def main():
    if sys.argv[1:] != EXPECTED:
        raise ValueError("unreviewed_arguments")
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        if ready():
            # No shell, background model, retry or import before limits pass.
            os.execv("/usr/local/bin/vllm", ["/usr/local/bin/vllm", "serve", *EXPECTED])
        time.sleep(0.1)
    raise ValueError("resource_admission_expired")


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Owned reasoning startup refused: resource admission or arguments unavailable.", file=sys.stderr)
        raise SystemExit(1)
