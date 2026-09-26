"""Installer primitives. No runtime imports, credentials or executable manifests."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat


def plain(path):
    path = Path(os.path.abspath(path))
    for node in (path, *path.parents):
        if node.exists() or node.is_symlink():
            value = node.lstat()
            if stat.S_ISLNK(value.st_mode) or getattr(value, "st_file_attributes", 0) & 0x400:
                raise ValueError("Symlink/reparse point is not an installation input")
    return path


def read(path, limit):
    path = plain(path)
    info = path.stat()
    if not stat.S_ISREG(info.st_mode) or info.st_size > limit:
        raise ValueError("Invalid or oversized installation file")
    with path.open("rb") as source:
        value = source.read(limit + 1)
    if len(value) > limit:
        raise ValueError("Installation file grew")
    return value


def digest(path):
    path = plain(path)
    before = path.stat()
    if not stat.S_ISREG(before.st_mode) or before.st_size > 1_073_741_824:
        raise ValueError("Invalid package member")
    result = hashlib.sha256()
    count = 0
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            count += len(chunk)
            if count > before.st_size:
                raise ValueError("Package member changed")
            result.update(chunk)
    after = path.stat()
    if count != before.st_size or (before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_ino, after.st_size, after.st_mtime_ns):
        raise ValueError("Package member changed")
    return result.hexdigest()


def sha(value):
    if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
        raise ValueError("An exact lowercase SHA256 is required")
    return value


def relative(value):
    if not isinstance(value, str) or not value or len(value) > 240 or "\\" in value or ":" in value:
        raise ValueError("Invalid package path")
    path = PurePosixPath(value)
    if path.is_absolute() or str(path) != value or any(part in {"", ".", ".."} or part.endswith((".", " ")) for part in path.parts):
        raise ValueError("Invalid package path")
    reserved = {"CON", "PRN", "AUX", "NUL", *("COM" + str(v) for v in range(1, 10)), *("LPT" + str(v) for v in range(1, 10))}
    if any(part.split(".")[0].upper() in reserved for part in path.parts):
        raise ValueError("Reserved Windows device name")
    if any(ord(c) < 32 for c in value):
        raise ValueError("Invalid package path")
    return path


def exclusive_json(path, value):
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    fd = os.open(plain(path), os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, "wb") as output:
        output.write(encoded)
        output.flush()
        os.fsync(output.fileno())


def atomic_json(path, value):
    temporary = path.with_name(path.name + ".new")
    exclusive_json(temporary, value)
    os.replace(temporary, plain(path))


def files(root):
    root = plain(root)
    result = []
    pending = [root]
    entries = 0
    while pending:
        for path in pending.pop().iterdir():
            entries += 1
            if entries > 1024:
                raise ValueError("Package entry limit")
            plain(path)
            if path.is_dir():
                pending.append(path)
            elif path.is_file():
                result.append(path.relative_to(root).as_posix())
            else:
                raise ValueError("Unsupported package entry")
    return sorted(result)


def verify(root, expected, extras=()):
    root = plain(root)
    if digest(root / "package.json") != sha(expected):
        raise ValueError("Package manifest SHA256 mismatch")
    manifest = json.loads(read(root / "package.json", 262144))
    if set(manifest) != {"version", "source", "store_schema", "files"} or manifest["version"] != 1 or manifest["store_schema"] not in (13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29):
        raise ValueError("Unsupported package manifest")
    sha(manifest["source"])
    members = manifest["files"]
    if not isinstance(members, list) or not 4 <= len(members) <= 512:
        raise ValueError("Package member limit")
    names = set()
    size = 0
    for member in members:
        if set(member) != {"path", "bytes", "sha256"}:
            raise ValueError("Unsupported package member")
        name = str(relative(member["path"]))
        if name.casefold() in names or name == "package.json":
            raise ValueError("Duplicate package member")
        names.add(name.casefold())
        path = plain(root / name)
        if type(member["bytes"]) is not int or member["bytes"] < 0 or path.stat().st_size != member["bytes"] or digest(path) != sha(member["sha256"]):
            raise ValueError("Package member hash/length mismatch")
        size += member["bytes"]
        if size > 1_073_741_824:
            raise ValueError("Package byte limit")
    if not {"avesra-desktop.exe", "avesra-native-host.exe", "extension/manifest.json", "runbook.md", "install-windows.py", "package_common.py"} <= names:
        raise ValueError("Incomplete package")
    if set(files(root)) != {member["path"] for member in members} | {"package.json"} | set(extras):
        raise ValueError("Unexpected package file")
    return manifest
