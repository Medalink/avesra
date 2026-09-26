"""Model folder integrity: a read-only mount and a pinned content manifest per lane.

Model libraries trust their folder completely (configs can name code or remote kernels, checkpoint
indexes name files to open), so every file is hashed before any loader sees the folder, and the mount
must be read-only so the verified bytes cannot change before or during loading.
"""
import hashlib
import json
import os
from pathlib import Path

# SHA-256 of each lane's canonical manifest at its pinned revision. Produce it on the Spark with
# `python -m avesra_audio.admin manifest --lane <lane> --model <dir>` against the read-only mount,
# then pin the printed digest here. An unpinned lane refuses to load.
MANIFESTS = {
    "speaker": None,
    "asr": None,
    "tts": None,
    "voice-design": None,
}
# Hub download metadata at the folder root; no loader reads it.
SKIP_ROOT = {".cache"}
CHUNK = 8 * 1024 * 1024


def read_only(path):
    return bool(os.statvfs(path).f_flag & os.ST_RDONLY)


def _hash(path):
    digest = hashlib.sha256()
    size = 0
    with open(path, "rb") as source:
        while chunk := source.read(CHUNK):
            digest.update(chunk)
            size += len(chunk)
    return size, digest.hexdigest()


def manifest(root):
    """Every file under root as sorted {path, size, sha256}; rejects anything a loader could be redirected by."""
    root = Path(root).resolve(strict=True)
    if not root.is_dir():
        raise ValueError("model_folder_unavailable")
    entries = []
    for directory, dirs, files in os.walk(root, followlinks=False):
        here = Path(directory)
        if here == root:
            dirs[:] = [name for name in dirs if name not in SKIP_ROOT]
        dirs.sort()
        if any((here / name).is_symlink() for name in dirs):
            raise ValueError("model_symlinked_folder")
        for name in sorted(files):
            path = here / name
            # Hub cache layouts symlink files to blobs; the target is hashed and must be read-only too.
            target = path.resolve(strict=True) if path.is_symlink() else path
            if not target.is_file():
                raise ValueError("model_special_file")
            if not read_only(target):
                raise ValueError("model_mount_writable")
            size, digest = _hash(target)
            entries.append({"path": path.relative_to(root).as_posix(), "size": size, "sha256": digest})
    if not entries:
        raise ValueError("model_folder_empty")
    return entries


def digest(entries):
    return hashlib.sha256(json.dumps(entries, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def verify(lane, root):
    expected = MANIFESTS.get(lane)
    if expected is None:
        raise ValueError("model_manifest_unpinned")
    if not read_only(root):
        raise ValueError("model_mount_writable")
    if digest(manifest(root)) != expected:
        raise ValueError("model_manifest_mismatch")
