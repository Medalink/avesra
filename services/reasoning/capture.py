"""Explicit pre-load artifact evidence; never stops or starts Local Studio.

Run only while the selected Local Studio engine is stopped:
  python3 capture.py /private/avesra/reasoning.json
The output is a private observation record, not a configuration readiness flag.
"""
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import time
import urllib.request

from profiles import profile


def read_bounded(path, limit):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as source:
        if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
            raise ValueError("non_regular_input")
        data = source.read(limit + 1)
    if len(data) > limit:
        raise ValueError("file_limit")
    return data


def stamp(path):
    stat = path.lstat()
    if not path.is_file() or path.is_symlink():
        raise ValueError("non_regular_artifact")
    return [stat.st_dev, stat.st_ino, stat.st_size, stat.st_ctime_ns]


def absent(instance):
    ids = subprocess.check_output(
        ["/usr/bin/docker", "ps", "--filter", "name=^/local-studio-" + instance + "$", "--format", "{{.ID}}"],
        timeout=5,
    )
    if ids.strip():
        raise ValueError("stop_selected_engine_through_local_studio_first")


def capture():
    config_path = Path(sys.argv[1]).absolute()
    if config_path.stat().st_mode & 0o077 or config_path.stat().st_size > 8192:
        raise ValueError("private_config_required")
    config = json.loads(read_bounded(config_path, 8192))
    name = config["engine"].get("profile", "legacy_35b")
    selected = profile(name)
    revision, repository = selected["revision"], selected["repository"]
    if (config["version"] != 2 or config["artifact_revision"] != revision
            or config["engine"]["image"] != selected["image"]
            or config["expected"]["recipe"] != selected["model"]
            or config["expected"]["model"] != selected["model"]
            or config["expected"]["port"] != selected["port"]):
        raise ValueError("unsupported_deployment")
    instance = config["expected"]["instance"]
    if not isinstance(instance, str) or not 1 <= len(instance) <= 128 or any(
            not (c.isascii() and (c.isalnum() or c in "-_.")) for c in instance):
        raise ValueError("instance_identity")
    root = Path(config["engine"]["model_directory"]).resolve(strict=True)
    absent(instance)
    # Only immutable public upstream metadata is fetched; no model download.
    url = f"https://huggingface.co/api/models/{repository}/revision/{revision}?blobs=true"
    with urllib.request.urlopen(url, timeout=30) as response:
        data = response.read(4_194_305)
        if response.url != url or len(data) > 4_194_304:
            raise ValueError("artifact_metadata_limit")
    metadata = json.loads(data)
    if metadata.get("sha") != revision or metadata.get("id") != repository:
        raise ValueError("artifact_revision_changed")
    siblings = metadata["siblings"]
    if not 1 <= len(siblings) <= 256:
        raise ValueError("artifact_file_limit")
    files = {}
    total = 0
    for entry in siblings:
        name = entry["rfilename"]
        if name == ".gitattributes":
            continue
        if "/" in name or "\\" in name or name.startswith("."):
            raise ValueError("unsupported_artifact_layout")
        path = root / name
        before = stamp(path)
        total += before[2]
        if total > 64 * 1024**3:
            raise ValueError("artifact_size_limit")
        sha256 = hashlib.sha256()
        git = hashlib.sha1(b"blob " + str(before[2]).encode() + b"\0")
        count = 0
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(descriptor, "rb") as source:
            if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
                raise ValueError("non_regular_artifact")
            while block := source.read(1024 * 1024):
                count += len(block)
                if count > before[2]:
                    raise ValueError("artifact_grew_during_capture")
                sha256.update(block)
                git.update(block)
        if stamp(path) != before:
            raise ValueError("artifact_changed_during_capture")
        lfs = entry.get("lfs")
        if lfs:
            if lfs["sha256"] != sha256.hexdigest() or lfs["size"] != before[2]:
                raise ValueError("artifact_digest_mismatch")
        elif entry.get("blobId") != git.hexdigest():
            raise ValueError("artifact_blob_mismatch")
        files[name] = {"stamp": before, "sha256": sha256.hexdigest()}
    for filename, observed in files.items():
        if stamp(root / filename) != observed["stamp"]:
            raise ValueError("artifact_changed_before_capture_finished")
    names = set()
    for number, path in enumerate(root.iterdir()):
        if number >= 258:
            raise ValueError("artifact_directory_limit")
        if path.name not in {".cache", ".gitattributes"}:
            names.add(path.name)
    if names != set(files):
        raise ValueError("uncaptured_model_input")
    absent(instance)
    record = {"version": 1, "profile": config["engine"].get("profile", "legacy_35b"), "revision": revision, "repository": repository,
              "image": selected["image"], "root": str(root), "files": files,
              "boot": read_bounded(Path("/proc/sys/kernel/random/boot_id"), 128).decode().strip(),
              "captured_ns": time.time_ns()}
    destination = config_path.parent / "reasoning-artifact.json"
    # Never replace an earlier load's evidence implicitly.
    fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as output:
        json.dump(record, output, separators=(",", ":"))
        output.flush()
        os.fsync(output.fileno())
    print("Captured pinned artifact evidence. Start the selected model through Local Studio next.")


if __name__ == "__main__":
    try:
        capture()
    except Exception:
        # Errors never dump paths, configuration, HTTP bodies or credentials.
        print("Artifact capture failed; no qualification was issued.", file=sys.stderr)
        raise SystemExit(1)
