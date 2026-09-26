"""Private exact-container bridge. No lifecycle mutations or arbitrary commands."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import time

FAILURE = "helper_unavailable"


def read_bounded(path, limit):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as source:
        if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
            raise ValueError("non_regular_input")
        data = source.read(limit + 1)
    if len(data) > limit:
        raise ValueError("file_limit")
    return data


def run():
    global FAILURE
    encoded = sys.stdin.buffer.read(131073)
    if len(encoded) > 131072:
        raise ValueError("request_limit")
    request = json.loads(encoded)
    selected = profile(request["profile"])
    deadline = time.monotonic() + request["request"]["remaining_ms"] / 1000
    container = request["container"]
    instance = request["instance"]
    if not isinstance(instance, str) or not 1 <= len(instance) <= 128 or any(
            not (c.isascii() and (c.isalnum() or c in "-_.")) for c in instance):
        raise ValueError("instance_identity")
    if len(container) != 64 or any(c not in "0123456789abcdef" for c in container):
        raise ValueError("container_identity")
    record_path = Path(request["record"])
    FAILURE = "capture_missing"
    info = record_path.lstat()
    FAILURE = "capture_invalid"
    if record_path.is_symlink() or not record_path.is_file() or info.st_mode & 0o077 or info.st_size > 131072:
        raise ValueError("private_capture_required")
    raw = read_bounded(record_path, 131072)
    capture = json.loads(raw)
    if (capture.get("version") != 1 or capture.get("revision") != selected["revision"]
            or request["revision"] != selected["revision"] or capture.get("image") != selected["image"]
            or request["image"] != selected["image"] or capture.get("repository") != selected["repository"]
            or capture.get("profile", "legacy_35b") != request["profile"]
            or capture.get("boot") != read_bounded(Path("/proc/sys/kernel/random/boot_id"), 128).decode().strip()):
        raise ValueError("capture_binding")
    root = Path(request["root"]).resolve(strict=True)
    if str(root) != capture["root"]:
        raise ValueError("capture_root")
    files = capture["files"]
    if not isinstance(files, dict) or not 1 <= len(files) <= 256 or not all(
            name in files for name in ["config.json", "generation_config.json", "tokenizer.json", "tokenizer_config.json", "model.safetensors.index.json"]):
        raise ValueError("capture_files")

    def unchanged():
        global FAILURE
        FAILURE = "artifact_changed"
        # Unexpected model/tokenizer inputs must not be loaded outside capture.
        names = set()
        for number, path in enumerate(root.iterdir()):
            if number >= 258 or time.monotonic() >= deadline:
                raise ValueError("artifact_directory_limit")
            if path.name not in {".cache", ".gitattributes"}:
                names.add(path.name)
        if names != set(files):
            raise ValueError("uncaptured_model_input")
        for name, value in files.items():
            if not name or "/" in name or "\\" in name or name.startswith("."):
                raise ValueError("capture_filename")
            path = root / name
            stat = path.lstat()
            if path.is_symlink() or not path.is_file() or [stat.st_dev, stat.st_ino, stat.st_size, stat.st_ctime_ns] != value["stamp"]:
                raise ValueError("artifact_changed_since_observed_load")

    unchanged()
    FAILURE = "controlled_load_required"
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise ValueError("expired")
    data = subprocess.check_output(["/usr/bin/docker", "inspect", container], timeout=min(5, remaining))
    if len(data) > 262144:
        raise ValueError("container_metadata_limit")
    values = json.loads(data)
    if len(values) != 1:
        raise ValueError("container_ambiguity")
    value = values[0]
    def stamp(text):
        return int(datetime.datetime.fromisoformat(text.replace("Z", "+00:00")).timestamp() * 1_000_000_000)
    if (value["Id"] != container or value["Name"] != "/local-studio-" + instance or value["Image"] != selected["image"]
            or not value["State"]["Running"] or value["State"]["Restarting"]
            or value["Config"]["Cmd"] != selected["command"]
            or value["Config"]["Entrypoint"] != selected["entrypoint"]
            or value["HostConfig"]["NetworkMode"] not in {"default", "bridge"}
            or stamp(value["Created"]) <= capture["captured_ns"]
            or stamp(value["State"]["StartedAt"]) <= capture["captured_ns"]):
        raise ValueError("controlled_new_load_required")
    if selected["guard"] is not None:
        limits = value["HostConfig"]
        if (not 0 < limits["Memory"] <= 36 * 1024**3
                or limits["MemorySwap"] != limits["Memory"]
                or not 0 < limits["NanoCpus"] <= 2_000_000_000):
            raise ValueError("owned_resource_limits_changed")
    mounts = [m for m in value["Mounts"] if m["Destination"] == "/models"]
    if (len(mounts) != 1 or mounts[0]["Type"] != "bind" or mounts[0]["RW"]
            or mounts[0]["Source"] != str(root)):
        raise ValueError("readonly_model_mount_required")
    # Reject overlays under the model mount, which could change loaded inputs.
    if len(value["Mounts"]) != 1:
        raise ValueError("model_overlay")
    remaining_ms = int((deadline - time.monotonic()) * 1000)
    if remaining_ms <= 0:
        raise ValueError("expired")
    request["request"]["remaining_ms"] = remaining_ms
    request["request"]["started_ns"] = stamp(value["State"]["StartedAt"])
    FAILURE = "helper_unavailable"
    result = subprocess.run(["/usr/bin/docker", "exec", "-i", container, "python3", "-c", sys.argv[1]],
                            input=json.dumps(request["request"]).encode(), stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, timeout=remaining_ms / 1000)
    if len(result.stdout) > 1500000 or time.monotonic() >= deadline:
        raise ValueError("engine_operation_unavailable")
    observed = json.loads(result.stdout)
    if result.returncode:
        if observed in [{"error": reason} for reason in ["engine_identity", "context_capacity", "stream_drain"]]:
            print(json.dumps(observed))
            raise SystemExit(1)
        raise ValueError("invalid_engine_failure")
    unchanged()
    quality = observed.get("quality")
    if quality is not None:
        try:
            if not isinstance(quality, str) or len(quality) != 64 or any(c not in "0123456789abcdef" for c in quality):
                raise ValueError("quality_digest")
            environment = {}
            for item in value["Config"]["Env"]:
                name, separator, content = item.partition("=")
                if not separator or not name or name in environment:
                    raise ValueError("environment_shape")
                environment[name] = content
            # Only this reviewed API authentication value is algorithm-independent.
            environment.pop("VLLM_API_KEY", None)
            package = {}
            for name, entry in files.items():
                content = entry["sha256"]
                if not isinstance(content, str) or len(content) != 64 or any(c not in "0123456789abcdef" for c in content):
                    raise ValueError("capture_content_hash")
                package[name] = content
            descriptor = {"version": 1, "engine": quality, "image": value["Image"],
                          "entrypoint": value["Config"]["Entrypoint"], "command": value["Config"]["Cmd"],
                          "environment": environment, "capacity": selected["capacity"], "package": package,
                          "repository": capture["repository"], "revision": capture["revision"]}
            observed["quality"] = hashlib.sha256(json.dumps(descriptor, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        except (KeyError, TypeError, ValueError):
            observed["quality"] = None
    observed["artifact"] = hashlib.sha256(raw).hexdigest()
    print(json.dumps(observed, separators=(",", ":")))


if __name__ == "__main__":
    try:
        run()
    except Exception:
        print(json.dumps({"error": FAILURE}))
        raise SystemExit(1)
