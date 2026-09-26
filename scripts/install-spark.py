"""Owned per-user controller installation. Explicit activation; never initializes data."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import ssl
import subprocess
import sys
import time

sys.dont_write_bytecode = True
from package_common import atomic_json, digest, exclusive_json, plain, read, sha

UNIT = "avesra-controller.service"
HEADER = "# Avesra owned controller unit v1\n"


def systemctl(*arguments):
    result = subprocess.run(["/usr/bin/systemctl", "--user", *arguments], stdin=subprocess.DEVNULL,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=25)
    if result.returncode:
        raise ValueError("Owned user-service operation failed")


def unit_path():
    # systemd's standard per-user path; no arbitrary unit names or root unit.
    return plain(Path.home() / ".config/systemd/user" / UNIT)


def config_identity(path):
    path = plain(path)
    info = path.stat()
    if not path.is_dir() or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise ValueError("Existing private controller directory required")
    certificate = read(path / "server-cert.pem", 65536).decode("ascii")
    key = plain(path / "server-key.pem").stat()
    if key.st_uid != os.getuid() or key.st_mode & 0o077:
        raise ValueError("Existing private server key required")
    if not plain(path / "authentication.db").is_file():
        raise ValueError("Existing authentication store required; installation never initializes one")
    pin = hashlib.sha256(ssl.PEM_cert_to_DER_cert(certificate)).hexdigest()
    deployment = {}
    for member in path.iterdir():
        if member.name.endswith("-deployment.json") or member.name in {"reasoning.json", "reasoning-artifact.json"}:
            if len(deployment) >= 32 or member.stat().st_size > 262144:
                raise ValueError("Deployment metadata limit")
            deployment[member.name] = digest(member)
    return {"directory": str(path), "device": info.st_dev, "inode": info.st_ino,
            "certificate": pin, "deployment": deployment}


def quoted(path):
    value = str(path)
    if not value or any(ord(c) < 32 for c in value):
        raise ValueError("Invalid service path")
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"').replace("%", "%%").replace("$", "$$") + '"'


def definition(binary, directory):
    return (HEADER + "[Unit]\nDescription=Avesra private controller\nAfter=network-online.target\n"
            "Wants=network-online.target\nStartLimitIntervalSec=120\nStartLimitBurst=3\n"
            "[Service]\nType=simple\nExecStart=" + quoted(binary) + " serve " + quoted(directory) + " --manual-pairing\n"
            "Restart=on-failure\nRestartSec=10\nTimeoutStopSec=25\nKillMode=control-group\n"
            "MemoryMax=1G\nCPUQuota=200%\nNoNewPrivileges=yes\nUMask=0077\n"
            "StandardOutput=journal\nStandardError=journal\n[Install]\nWantedBy=default.target\n")


def package(root, identity):
    sha(identity)
    location = plain(root / "versions" / identity)
    metadata = json.loads(read(location / "package.json", 4096))
    if set(metadata) != {"version", "sha256", "source"} or metadata["version"] != 1 or metadata["sha256"] != identity:
        raise ValueError("Invalid retained controller package")
    sha(metadata["source"])
    if set(p.name for p in location.iterdir()) != {"avesra-server", "package.json"} or digest(location / "avesra-server") != identity:
        raise ValueError("Retained controller package changed")
    binary = plain(location / "avesra-server")
    info = binary.stat()
    if info.st_uid != os.getuid() or info.st_mode & 0o022 or not info.st_mode & 0o100:
        raise ValueError("Unsafe controller executable ownership/permissions")
    return binary


def stage(root, args):
    sha(args.source)
    source = plain(args.binary)
    if digest(source) != sha(args.sha256):
        raise ValueError("Controller SHA256 mismatch")
    with source.open("rb") as executable:
        header = executable.read(20)
    if len(header) != 20 or header[:6] != b"\x7fELF\x02\x01" or int.from_bytes(header[18:20], "little") != 183:
        raise ValueError("Expected a Linux ARM64 executable")
    target = root / "versions" / args.sha256
    if target.exists():
        package(root, args.sha256)
        print("Controller already staged:", args.sha256)
        return
    temporary = target.with_name(target.name + ".staging")
    temporary.mkdir(mode=0o700)
    shutil.copyfile(source, temporary / "avesra-server")
    (temporary / "avesra-server").chmod(0o700)
    if digest(temporary / "avesra-server") != args.sha256:
        raise ValueError("Controller changed during copy")
    exclusive_json(temporary / "package.json", {"version": 1, "sha256": args.sha256, "source": args.source})
    os.rename(temporary, target)
    package(root, args.sha256)
    print("Controller staged; no service or deployment was changed:", args.sha256)


def health(pin):
    deadline = time.monotonic() + 30
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    # Exact existing certificate pin replaces public-CA name validation here.
    while time.monotonic() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", 9474), timeout=min(2, deadline - time.monotonic())) as connection:
                with context.wrap_socket(connection, server_hostname="localhost") as tls:
                    if hashlib.sha256(tls.getpeercert(binary_form=True)).hexdigest() != pin:
                        raise ValueError("Existing controller certificate changed")
                    tls.sendall(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    data = bytearray()
                    while True:
                        if time.monotonic() >= deadline or len(data) > 16384:
                            raise ValueError("Health response expired/oversized")
                        tls.settimeout(min(2, deadline - time.monotonic()))
                        block = tls.recv(min(4096, 16385 - len(data)))
                        if not block:
                            break
                        data.extend(block)
                    header, body = bytes(data).split(b"\r\n\r\n", 1)
                    if not header.startswith(b"HTTP/1.1 200 ") or json.loads(body).get("product") != "Avesra":
                        raise ValueError("Unexpected controller health response")
                    systemctl("is-active", "--quiet", UNIT)
                    return
        except (OSError, TimeoutError):
            time.sleep(min(0.5, max(0, deadline - time.monotonic())))
    raise ValueError("Controller health did not become available within30seconds")


def active(root):
    path = root / "active.json"
    if not path.exists():
        return None
    record = json.loads(read(path, 16384))
    if set(record) != {"version", "current", "previous", "configuration", "unit_sha256"} or record["version"] != 1:
        raise ValueError("Invalid controller ownership record")
    binary = package(root, record["current"])
    expected = definition(binary, record["configuration"]["directory"]).encode()
    if hashlib.sha256(expected).hexdigest() != record["unit_sha256"]:
        raise ValueError("Unexpected owned unit definition")
    return record


def write_unit(text):
    path = unit_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".new")
    fd = os.open(temporary, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, "w") as output:
        output.write(text)
        output.flush()
        os.fsync(output.fileno())
    os.replace(temporary, path)


def restore(root, journal):
    old = journal["old"]
    # Never replace an externally edited unit even during recovery.
    present = read(unit_path(), 16384) if unit_path().exists() else None
    permitted = [journal["new_unit"].encode()]
    if journal["old_unit"] is not None:
        permitted.append(journal["old_unit"].encode())
    else:
        permitted.append(None)
    if present not in permitted:
        raise ValueError("Unit changed externally; recovery refused")
    if old is not None:
        package(root, old["current"])
        if config_identity(old["configuration"]["directory"]) != old["configuration"]:
            raise ValueError("Configuration changed; automatic rollback refused")
    systemctl("stop", UNIT)
    if old is None:
        systemctl("disable", UNIT)
        if unit_path().exists():
            unit_path().unlink()
        systemctl("daemon-reload")
        if (root / "active.json").exists():
            (root / "active.json").unlink()
    else:
        write_unit(journal["old_unit"])
        systemctl("daemon-reload")
        systemctl("reset-failed", UNIT)
        systemctl("enable", "--now", UNIT)
        health(old["configuration"]["certificate"])
        atomic_json(root / "active.json", old)
    (root / "activation.json").unlink()


def activate(root, args):
    old = active(root)
    path = unit_path()
    prior_unit = read(path, 16384).decode() if path.exists() else None
    if old is None and prior_unit is not None or old is not None and (prior_unit is None or hashlib.sha256(prior_unit.encode()).hexdigest() != old["unit_sha256"]):
        raise ValueError("Existing user unit is unrelated or changed")
    if old is None:
        # Do not steal the existing transient controller's port or stop it.
        with socket.socket() as probe:
            try:
                probe.bind(("0.0.0.0", 9474))
            except OSError:
                raise ValueError("An existing listener owns9474; its owner must retire it separately") from None
    identity = old["previous"] if args.command == "rollback" and old else getattr(args, "sha256", None)
    if identity is None:
        raise ValueError("No retained prior controller")
    binary = package(root, identity)
    config = config_identity(args.config if args.command == "activate" else old["configuration"]["directory"])
    if old is not None and config != old["configuration"]:
        raise ValueError("Existing configuration identity changed; explicit deployment review required")
    text = definition(binary, config["directory"])
    new = {"version": 1, "current": identity, "previous": old["current"] if old else None,
           "configuration": config, "unit_sha256": hashlib.sha256(text.encode()).hexdigest()}
    journal = {"version": 1, "old": old, "old_unit": prior_unit, "new_unit": text}
    exclusive_json(root / "activation.json", journal)
    try:
        write_unit(text)
        if config_identity(config["directory"]) != config:
            raise ValueError("Configuration changed during activation")
        systemctl("daemon-reload")
        systemctl("enable", UNIT)
        systemctl("restart", UNIT)
        health(config["certificate"])
        if config_identity(config["directory"]) != config:
            raise ValueError("Configuration changed during health observation")
        atomic_json(root / "active.json", new)
        (root / "activation.json").unlink()
    except Exception:
        restore(root, journal)
        raise ValueError("Activation failed; prior verified controller restored (or fresh unit removed)") from None
    print("Persistent user controller activated with pinned-certificate health.")
    print("Starts at user login. Boot without login requires separately configured user linger.")
    print("Audio/model configuration was preserved; health does not qualify inference.")


def main():
    if sys.platform != "linux" or os.getuid() == 0 or sys.version_info < (3, 12):
        raise ValueError("Use Python3.12+ as the existing non-root Spark service user")
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    staged = commands.add_parser("stage")
    for name in ("binary", "sha256", "source"):
        staged.add_argument("--" + name, required=True)
    selected = commands.add_parser("activate")
    selected.add_argument("--sha256", required=True)
    selected.add_argument("--config", required=True)
    commands.add_parser("rollback")
    commands.add_parser("recover")
    args = parser.parse_args()
    root = plain(Path.home() / ".local/share/avesra-install")
    root.mkdir(parents=True, exist_ok=True, mode=0o700)
    if root.stat().st_uid != os.getuid() or root.stat().st_mode & 0o077:
        raise ValueError("Private installer directory required")
    (root / "versions").mkdir(exist_ok=True, mode=0o700)
    with (root / "installer.lock").open("a+b") as owner:
        fcntl.flock(owner, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if (root / "activation.json").exists() and args.command != "recover":
            raise ValueError("Interrupted activation: run recover first")
        if args.command == "stage":
            stage(root, args)
        elif args.command == "recover":
            journal = json.loads(read(root / "activation.json", 32768))
            if set(journal) != {"version", "old", "old_unit", "new_unit"} or journal["version"] != 1:
                raise ValueError("Invalid recovery journal")
            restore(root, journal)
        else:
            activate(root, args)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print("Controller installation refused:", error, file=sys.stderr)
        raise SystemExit(1)
