"""Fixed reviewed deployment descriptors, bundled into both private helpers."""
LEGACY_IMAGE = "sha256:d464f3b466fa9c45ddbff8a812e80564503b6879a9fd95c1a47514f3f0df5a4a"
OWNED_IMAGE = "sha256:d7c6827ade3234f66b365a492ff81e927ed64fc1c73f180aaefab69ebf8c2dd0"
PROFILES = {
    "legacy_35b": {
        "image": LEGACY_IMAGE, "revision": "95a723d08a9490559dae23d0cff1d9466213d989",
        "repository": "Qwen/Qwen3.6-35B-A3B-FP8", "model": "avesra-fast", "port": 8888, "capacity": 16384,
        "entrypoint": ["vllm", "serve"], "guard": None,
        "command": ["/models", "--served-model-name", "avesra-fast", "--host", "0.0.0.0", "--port", "8888",
                    "--max-model-len", "16384", "--gpu-memory-utilization", "0.5", "--max-num-seqs", "2",
                    "--tool-call-parser", "qwen3_coder", "--enable-auto-tool-choice", "--reasoning-parser", "qwen3",
                    "--max-num-batched-tokens", "2048", "--limit-mm-per-prompt", '{"image":1,"video":0}'],
    },
    "owned_27b": {
        "image": OWNED_IMAGE, "revision": "017b9c7af6b5689d5dd426a76e0bc077eb5ca20a",
        "repository": "Qwen/Qwen3.8-27B-FP8", "model": "avesra-owned-27b", "port": 8000, "capacity": 4096,
        "entrypoint": ["python3", "/opt/avesra/bounded_start.py"],
        "guard": "209e4905b1176aab52ca35cb2b8af04e8f053628c03e1090b2645aa6f8316d19",
        "command": ["/models", "--served-model-name", "avesra-owned-27b", "--host", "0.0.0.0", "--port", "8000",
                    "--max-model-len", "4096", "--gpu-memory-utilization", "0.25", "--max-num-seqs", "1",
                    "--reasoning-parser", "qwen3", "--max-num-batched-tokens", "512", "--kv-cache-memory-bytes",
                    "805306368", "--enforce-eager", "--language-model-only"],
    },
}


def profile(name):
    if not isinstance(name, str) or name not in PROFILES:
        raise ValueError("unsupported_profile")
    return PROFILES[name]
