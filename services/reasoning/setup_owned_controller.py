"""Explicit operator setup of an isolated controller copy; no existing service edits.

Preparation writes only a new owned directory and a new user-unit definition.
--start additionally starts that one bounded user service; no model is launched.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import stat
import subprocess
import sys

PINNED = {
    "src/modules/system/platform/gpu.ts": "4a38399da69e3e7a7803c839646171fa85a99a06573055a536bf652709a85afe",
    "src/config/env.ts": "10fb0957a8f5ac9266061a7218b78a7fbb382180c298237a162cdae88bdb224b",
    "src/main.ts": "321b1cbbd15fd0c4237b064aed57633fbe87a7d09e49e08f70b43556780743ff",
    "src/modules/compute/lifecycle.ts": "7e7de129d78d347e746e79f95665181d28df459d5517ba116874990a292d28fd",
}
UNIT = "avesra-reasoning-owner.service"


def checksum(path):
    before = path.stat()
    if not stat.S_ISREG(before.st_mode) or before.st_size > 512 * 1024**2:
        raise ValueError("Unsupported controller input")
    value = hashlib.sha256()
    count = 0
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            count += len(block)
            if count > before.st_size:
                raise ValueError("Controller input changed")
            value.update(block)
    after = path.stat()
    if (before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_ino, after.st_size, after.st_mtime_ns) or count != before.st_size:
        raise ValueError("Controller input changed")
    return value.hexdigest()


def quote(path):
    value = str(path)
    if any(c in value for c in '\n\r\x00'):
        raise ValueError("Invalid setup path")
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%').replace('$', '$$') + '"'


def unit_path_value(path):
    # WorkingDirectory/EnvironmentFile are path directives, not ExecStart argv.
    # Keep this operator recipe narrow rather than inventing parser escaping.
    value = str(path)
    if not value.startswith("/") or any(not (c.isascii() and (c.isalnum() or c in "/._-")) for c in value):
        raise ValueError("Unit paths must be absolute ASCII paths without spaces")
    return value


def prepare(args):
    if sys.platform != "linux" or os.getuid() == 0:
        raise ValueError("Run as the existing non-root service user")
    os.umask(0o077)
    source = Path(args.source).resolve(strict=True)
    destination = Path(args.destination).absolute()
    if destination.exists() or destination.is_symlink() or source == destination or source in destination.parents:
        raise ValueError("Use a new owned directory outside the Local Studio checkout")
    for parent in destination.parents:
        if parent.is_symlink():
            raise ValueError("Symlink destination parent")
    unit = Path.home() / ".config/systemd/user" / UNIT
    if unit.exists() or unit.is_symlink():
        raise ValueError("Owned unit already exists; do not overwrite it implicitly")
    for relative, expected in PINNED.items():
        if checksum(source / relative) != expected:
            raise ValueError("Reviewed controller source has changed")
    bun = Path(args.bun).resolve(strict=True)
    model = Path(args.models).resolve(strict=True)
    if not model.is_dir():
        raise ValueError("Existing models directory required")
    # dotenv searches the working directory and two parents. Do not import a
    # user's environment file into the isolated owner, even as unused defaults.
    for parent in (destination, destination.parent):
        if (parent / ".env").exists():
            raise ValueError("Environment file in owned controller ancestry")
    destination.mkdir(mode=0o700, parents=False)
    code = destination / "controller"
    code.mkdir(mode=0o700)
    records = []
    total = 0

    def copy_tree(path, target, boundary, ancestors=()):
        nonlocal total
        actual = path.resolve(strict=True)
        if actual != boundary and boundary not in actual.parents:
            raise ValueError("Dependency escapes its copied tree")
        if actual in ancestors:
            raise ValueError("Cyclic dependency input")
        if actual.is_dir():
            target.mkdir(mode=0o700)
            for child in sorted(actual.iterdir()):
                copy_tree(child, target / child.name, boundary, (*ancestors, actual))
            return
        if len(records) >= 50000:
            raise ValueError("Controller file count limit")
        total += actual.stat().st_size
        if total > 1024**3:
            raise ValueError("Controller copy byte limit")
        before = checksum(actual)
        shutil.copyfile(actual, target)
        if checksum(target) != before or checksum(actual) != before:
            raise ValueError("Controller input changed during copy")
        target.chmod(0o400)
        records.append({"path": target.relative_to(destination).as_posix(), "sha256": before, "bytes": target.stat().st_size})

    for member in ("src", "contracts", "node_modules"):
        copy_tree(source / member, code / member, (source / member).resolve(strict=True))
    copy_tree(source / "package.json", code / "package.json", source)
    copy_tree(source / "tsconfig.json", code / "tsconfig.json", source)
    runtime = destination / "bun"
    copy_tree(bun, runtime, bun.parent)
    runtime.chmod(0o500)
    changed = code / "src/modules/system/platform/gpu.ts"
    before = checksum(changed)
    text = changed.read_text()
    old = "    memory_free_mb: memoryFreeMb,\n"
    if text.count(old) != 1:
        raise ValueError("Reviewed telemetry patch no longer applies")
    changed.chmod(0o600)
    changed.write_text(text.replace(old, old + "    memory_shared: isUnifiedMemoryNvidia,\n"))
    changed.chmod(0o400)
    after = checksum(changed)
    for entry in records:
        if entry["path"] == changed.relative_to(destination).as_posix():
            entry["sha256"] = after
            entry["bytes"] = changed.stat().st_size
    data = destination / "data"
    data.mkdir(mode=0o700)
    credentials = destination / "controller.env"
    environment = {
        "LOCAL_STUDIO_HOST": "127.0.0.1", "LOCAL_STUDIO_PORT": "18080",
        "LOCAL_STUDIO_DATA_DIR": str(data), "LOCAL_STUDIO_DB_PATH": str(data / "controller.db"),
        "LOCAL_STUDIO_MODELS_DIR": str(model), "LOCAL_STUDIO_DISABLE_METRICS": "true",
        "LOCAL_STUDIO_INFERENCE_HOST": "127.0.0.1", "LOCAL_STUDIO_INFERENCE_PORT": "8000",
        "LOCAL_STUDIO_API_KEY": secrets.token_hex(32), "INFERENCE_API_KEY": secrets.token_hex(32),
    }
    with credentials.open("x") as output:
        for key, value in environment.items():
            # All path values are fixed native setup paths, not shell commands.
            if any(c in value for c in '\n\r"\\'):
                raise ValueError("Unsupported environment path")
            output.write(key + '="' + value + '"\n')
    credentials.chmod(0o600)
    revision = subprocess.check_output(["/usr/bin/git", "-C", str(source), "rev-parse", "HEAD"], timeout=5).decode().strip()
    manifest = {"version": 1, "upstream_revision": revision, "files": records,
                "delta": {"path": "controller/src/modules/system/platform/gpu.ts", "before": before, "after": after,
                          "change": "Expose existing isUnifiedMemoryNvidia as memory_shared"}}
    encoded = json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
    (destination / "source-manifest.json").write_bytes(encoded)
    (destination / "source-manifest.json").chmod(0o400)
    text = ("# Avesra isolated Local Studio owner; existing controller remains unchanged\n"
            "[Unit]\nDescription=Avesra isolated reasoning deployment owner\n"
            "StartLimitIntervalSec=120\nStartLimitBurst=3\n[Service]\nType=simple\n"
            "WorkingDirectory=" + unit_path_value(code) + "\nExecStart=" + quote(runtime) + " src/main.ts\n"
            "EnvironmentFile=" + unit_path_value(credentials) + "\nEnvironment=PATH=/usr/bin:/bin\n"
            "MemoryMax=512M\nCPUQuota=100%\nNoNewPrivileges=yes\nUMask=0077\n"
            "ProtectSystem=strict\nProtectHome=read-only\nReadWritePaths=" + unit_path_value(data) + "\n"
            "PrivateTmp=yes\nRestart=on-failure\nRestartSec=10\nTimeoutStopSec=20\n"
            "[Install]\nWantedBy=default.target\n")
    unit.parent.mkdir(parents=True, exist_ok=True)
    with unit.open("x") as output:
        output.write(text)
    unit.chmod(0o600)
    print("Owned controller copy prepared. Source manifest SHA256:", hashlib.sha256(encoded).hexdigest())
    print("Private credential file created; values were not printed. No model launched.")
    if args.start:
        for options in (("daemon-reload",), ("start", UNIT)):
            subprocess.run(["/usr/bin/systemctl", "--user", *options], check=True, timeout=25,
                           stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        print("Started only", UNIT, "on loopback18080 with512MiB/1CPU limits.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True)
    parser.add_argument("--destination", required=True)
    parser.add_argument("--bun", required=True)
    parser.add_argument("--models", required=True)
    parser.add_argument("--start", action="store_true")
    try:
        prepare(parser.parse_args())
    except Exception:
        print("Owned controller setup failed. Partial owned files are retained; original service untouched.", file=sys.stderr)
        raise SystemExit(1)
