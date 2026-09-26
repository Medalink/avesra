"""Explicit persistent ownership of one existing Avesra audio container.

Does not recreate containers, mutate deployment files, load models or retry inference.
"""
import argparse
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import select
import shutil
import signal
import socket
import stat
import struct
import subprocess
import sys
import time

sys.dont_write_bytecode = True
from package_common import atomic_json, digest, exclusive_json, plain, read, sha

LANES = {"asr", "speaker", "tts", "voice-design", "activity"}
STOP = False


def command(arguments, timeout=10, limit=262144):
    child = subprocess.Popen(arguments, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    deadline = time.monotonic() + timeout
    output = bytearray()
    try:
        while True:
            left = deadline - time.monotonic()
            if left <= 0:
                raise ValueError("Owned metadata operation expired")
            if not select.select([child.stdout], [], [], left)[0]:
                raise ValueError("Owned metadata operation expired")
            block = os.read(child.stdout.fileno(), min(65536, limit + 1 - len(output)))
            if not block:
                break
            output.extend(block)
            if len(output) > limit:
                raise ValueError("Owned metadata output limit")
        if child.wait(timeout=max(0.01, deadline - time.monotonic())):
            raise ValueError("Owned container operation failed")
        return bytes(output)
    finally:
        if child.poll() is None:
            child.kill()
        child.wait()
        child.stdout.close()


def inspect(container):
    if not re.fullmatch(r"[0-9a-f]{64}", container):
        raise ValueError("Exact full container ID required")
    values = json.loads(command(["/usr/bin/docker", "inspect", container]))
    if not isinstance(values, list) or len(values) != 1 or values[0]["Id"] != container:
        raise ValueError("Container identity changed")
    return values[0]


def fingerprint(value):
    # Hash potentially private environment values without persisting or logging them.
    selected = {key: value[key] for key in ("Id", "Name", "Image", "Created", "Path", "Args", "Config", "HostConfig", "Mounts")}
    host = dict(selected["HostConfig"])
    oom = host["OomKillDisable"]
    if oom is not None and type(oom) is not bool:
        raise ValueError("Invalid audio OOM policy")
    # Docker defaults nil to false, and may clear false on first start when the
    # kernel lacks this optional knob. Never erase an explicit true policy.
    host["OomKillDisable"] = False if oom is None else oom
    selected["HostConfig"] = host
    mounts = selected["Mounts"]
    if not isinstance(mounts, list) or any(
        not isinstance(mount, dict) or not isinstance(mount.get("Destination"), str)
        or not mount["Destination"].startswith("/") for mount in mounts
    ):
        raise ValueError("Invalid audio mount identity")
    if len({mount["Destination"] for mount in mounts}) != len(mounts):
        raise ValueError("Duplicate audio mount destination")
    # Docker emits this set in nondeterministic order across fresh inspections.
    # Preserve every field and all other array ordering; never deduplicate mounts.
    selected["Mounts"] = sorted(mounts, key=lambda mount: mount["Destination"])
    return hashlib.sha256(json.dumps(selected, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def configuration(path):
    value = json.loads(read(path, 65536))
    lane, revision = value.get("lane"), value.get("model_revision")
    if lane not in LANES or not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("Exact audio lane and model revision required")
    if any(type(value.get(key, False)) is not bool for key in ("asr_streaming", "tts_streaming", "activity_streaming")):
        raise ValueError("Invalid streaming configuration")
    streaming = value.get("asr_streaming", False) or value.get("tts_streaming", False) or value.get("activity_streaming", False)
    return lane, revision, streaming


def current(record):
    if digest(record["config"]) != record["config_sha256"]:
        raise ValueError("Audio serving configuration changed")
    if record["deployment"] is not None and digest(record["deployment"]) != record["deployment_sha256"]:
        raise ValueError("Selected controller deployment changed")
    value = inspect(record["container"])
    if fingerprint(value) != record["container_sha256"]:
        raise ValueError("Exact audio container configuration changed")
    return value


def health(record):
    path = plain(record["socket"])
    info, parent = path.stat(), path.parent.stat()
    if not stat.S_ISSOCK(info.st_mode) or info.st_uid != os.getuid() or parent.st_uid != os.getuid() or parent.st_mode & 0o077:
        raise ValueError("Private same-user audio socket required")
    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(2)
        client.connect(str(path))
        if struct.unpack("3i", client.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))[1] != os.getuid():
            raise ValueError("Audio socket peer changed")
        request = b'{"version":1,"operation":"health"}'
        client.sendall(struct.pack("!I", len(request)) + request)
        def exact(length):
            value = bytearray()
            while len(value) < length:
                block = client.recv(length - len(value))
                if not block:
                    raise ValueError("Incomplete audio health")
                value.extend(block)
            return bytes(value)
        length = struct.unpack("!I", exact(4))[0]
        if not 0 < length <= 16384:
            raise ValueError("Audio health limit")
        value = json.loads(exact(length))
        if (value.get("version") != 1 or value.get("lane") != record["lane"]
                or value.get("model_revision") != record["revision"]
                or value.get("streaming") != record["streaming"]
                or value.get("permission_authority") is not False
                or value.get("cancellation") not in {"terminate_process", "cooperative_reset_or_terminate"}
                or (value.get("cancellation") == "cooperative_reset_or_terminate" and record["lane"] not in {"activity", "tts"})
                or value.get("state") not in {"unavailable", "loading", "loaded_unqualified", "termination_pending"}):
            raise ValueError("Audio health identity/contract changed")
        return value["state"]


def stopped(value):
    return not value["State"]["Running"] and not value["State"]["Restarting"]


def sync_directory(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def mark_operation(path, record, phase):
    writer = exclusive_json if phase == "start" else atomic_json
    writer(path, {"version": 1, "container": record["container"],
                  "container_sha256": record["container_sha256"], "phase": phase})
    sync_directory(path.parent)


def clear_operation(path):
    path.unlink()
    sync_directory(path.parent)


def stop_owned(container):
    # A timeout killing the docker client is not proof the container stopped.
    deadline = time.monotonic() + 35
    try:
        command(["/usr/bin/docker", "stop", "--time", "20", container], timeout=25, limit=128)
    except Exception:
        pass
    while time.monotonic() < deadline:
        try:
            value = inspect(container)
            if stopped(value):
                return True
        except Exception:
            pass  # The durable marker retains uncertainty beyond supervisor exit.
        time.sleep(1)
    return False


def start_owned(container):
    child = subprocess.Popen(["/usr/bin/docker", "start", container], stdin=subprocess.DEVNULL,
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    deadline = time.monotonic() + 20
    try:
        while child.poll() is None:
            if time.monotonic() >= deadline or STOP:
                raise ValueError("Owned audio start uncertain; durable admission remains blocked")
            time.sleep(0.1)
        if child.wait():
            raise ValueError("Owned audio start failed; durable admission remains blocked")
    finally:
        if child.poll() is None:
            child.kill()
        child.wait()


def retire_stale_socket(record):
    path = plain(record["socket"])
    if not path.exists():
        return
    before = path.stat()
    parent = path.parent.stat()
    if not stat.S_ISSOCK(before.st_mode) or before.st_uid != os.getuid() or parent.st_uid != os.getuid() or parent.st_mode & 0o077:
        raise ValueError("Unexpected stale socket identity")
    with socket.socket(socket.AF_UNIX) as probe:
        probe.settimeout(1)
        try:
            probe.connect(str(path))
        except OSError as error:
            if error.errno != errno.ECONNREFUSED:
                raise ValueError("Socket liveness uncertain; not removed") from None
        else:
            raise ValueError("Another process owns the audio socket")
    container = current(record)
    after = path.lstat()
    if container["State"]["Running"] or container["State"]["Restarting"] or (before.st_dev, before.st_ino, before.st_ctime_ns) != (after.st_dev, after.st_ino, after.st_ctime_ns):
        raise ValueError("Stale socket ownership changed")
    path.unlink()


def supervise(args):
    global STOP
    manifest = plain(args.manifest)
    if digest(manifest) != sha(args.sha256):
        raise ValueError("Audio owner manifest changed")
    record = json.loads(read(manifest, 16384))
    if record["version"] != 1 or digest(Path(__file__)) != record["supervisor_sha256"] or digest(Path(__file__).parent / "package_common.py") != record["common_sha256"]:
        raise ValueError("Owned supervisor code changed")
    value = current(record)
    pending = manifest.parent / "operation.json"
    if pending.with_name("operation.json.new").exists():
        raise ValueError("Interrupted operation journal write requires reconciliation")
    if pending.exists():
        operation = json.loads(read(pending, 4096))
        if (set(operation) != {"version", "container", "container_sha256", "phase"}
                or operation["version"] != 1 or operation["container"] != record["container"]
                or operation["container_sha256"] != record["container_sha256"]
                or operation["phase"] not in {"start", "owned", "stop"}):
            raise ValueError("Operation journal identity changed")
        if operation["phase"] == "start" or not stopped(value):
            raise ValueError("Previous audio operation is unsettled; no automatic replay")
        clear_operation(pending)
    if value["State"]["Running"] or value["State"]["Restarting"]:
        raise ValueError("Existing running container cannot be adopted; reconcile its owner first")
    retire_stale_socket(record)
    def stopping(*_):
        global STOP
        STOP = True
    signal.signal(signal.SIGTERM, stopping)
    signal.signal(signal.SIGINT, stopping)
    # Durable admission precedes the first possible daemon operation. A killed
    # client never clears an uncertain start, even if stop observes stopped now.
    mark_operation(pending, record, "start")
    started = False
    try:
        start_owned(record["container"])
        mark_operation(pending, record, "owned")
        started = True
        deadline = time.monotonic() + 30
        observed = None
        while not STOP and time.monotonic() < deadline:
            current(record)
            try:
                observed = health(record)
                break
            except (OSError, ValueError):
                time.sleep(0.5)
        if STOP:
            return
        if observed is None:
            raise ValueError("Audio socket health unavailable after30seconds")
        print("Owned audio process observed:", record["lane"], observed, "; voice qualification unchanged", flush=True)
        while not STOP:
            value = current(record)
            if not value["State"]["Running"]:
                raise ValueError("Owned audio process exited; inference outcomes are not retried")
            time.sleep(1)
    finally:
        transitioned = False
        try:
            if started:
                mark_operation(pending, record, "stop")
                transitioned = True
        finally:
            # Even a failed journal write cannot skip retiring our actual owner.
            # The preceding durable start/owned marker still blocks admission.
            settled = stop_owned(record["container"])
        if transitioned and settled:
            clear_operation(pending)
        else:
            raise ValueError("Audio operation unsettled; durable marker prevents replay")


def prepare(args):
    value = inspect(args.container)
    if value["Image"] != args.image or not re.fullmatch(r"sha256:[0-9a-f]{64}", args.image):
        raise ValueError("Immutable image ID mismatch")
    if not value["Name"].startswith("/avesra-") or value["HostConfig"]["RestartPolicy"]["Name"] not in {"", "no"}:
        raise ValueError("Only an explicitly owned Avesra container without another restart owner is supported")
    if (value["Config"]["Entrypoint"] != ["python3", "-m", "avesra_audio.service"]
            or value["Config"]["User"] != f"{os.getuid()}:{os.getgid()}"
            or value["HostConfig"]["Privileged"]):
        raise ValueError("Unsupported audio process principal/entrypoint")
    for mount in value["Mounts"]:
        target = mount["Destination"]
        permitted = target in {"/config.json", "/run/avesra", "/cache", "/voices", "/models"} or target.startswith("/models/")
        if mount["Type"] != "bind" or not permitted or (target == "/config.json" or target.startswith("/models")) and mount["RW"]:
            raise ValueError("Unreviewed audio overlay or writable model input")
    config = plain(args.config)
    lane, revision, streaming = configuration(config)
    host_socket = plain(args.socket)
    # Bind the actual fixed service argv to the mounted host config and socket.
    command_line = value["Config"]["Cmd"]
    if not isinstance(command_line, list) or len(command_line) != 4 or command_line[0] != "--config" or command_line[2] != "--socket":
        raise ValueError("Unsupported audio service command")
    def host_path(container_path):
        candidates = []
        for mount in value["Mounts"]:
            target = Path(mount["Destination"])
            selected = Path(container_path)
            if mount["Type"] == "bind" and (selected == target or target in selected.parents):
                candidates.append((len(target.parts), Path(mount["Source"]) / selected.relative_to(target)))
        if not candidates:
            raise ValueError("Audio service input is not an explicit bind mount")
        return plain(max(candidates, key=lambda v: v[0])[1])
    if host_path(command_line[1]) != config or host_path(command_line[3]) != host_socket:
        raise ValueError("Container config/socket differs from selected host paths")
    deployment = plain(args.deployment) if args.deployment else None
    if deployment:
        selection = json.loads(read(deployment, 4096))
        if set(selection) != {"socket", "model_revision"} or plain(selection["socket"]) != host_socket or selection["model_revision"] != revision:
            raise ValueError("Controller deployment does not select this exact audio service")
        if deployment.name != lane + "-deployment.json":
            raise ValueError("Wrong controller lane deployment")
    if args.enable and (deployment is None or lane == "voice-design"):
        raise ValueError("Optional/candidate voice-design services remain demand-started")
    if value["State"]["Running"]:
        # Read metadata only; preparation neither adopts nor terminates the service.
        observed = health({"socket": str(host_socket), "lane": lane, "revision": revision, "streaming": streaming})
        print("Preparation observed current unqualified state:", observed)
    container_sha256 = fingerprint(value)
    root = plain(Path.home() / ".local/share/avesra-install/audio" / args.container)
    root.mkdir(parents=True, mode=0o700, exist_ok=False)
    for name in ("install-audio.py", "package_common.py"):
        shutil.copyfile(Path(__file__).parent / name, root / name)
        (root / name).chmod(0o400)
    record = {"version": 1, "container": args.container, "image": args.image,
              "container_sha256": container_sha256, "config": str(config), "config_sha256": digest(config),
              "deployment": str(deployment) if deployment else None, "deployment_sha256": digest(deployment) if deployment else None,
              "socket": str(host_socket), "lane": lane, "revision": revision, "streaming": streaming,
              "supervisor_sha256": digest(root / "install-audio.py"), "common_sha256": digest(root / "package_common.py")}
    exclusive_json(root / "owner.json", record)
    identity = digest(root / "owner.json")
    unit_name = "avesra-audio-" + args.container[:16] + ".service"
    unit = plain(Path.home() / ".config/systemd/user" / unit_name)
    if unit.exists():
        raise ValueError("Audio unit already exists")
    if any(c.isspace() or c in '%$"\\' for c in str(root)):
        raise ValueError("Unsupported unit path")
    text = ("# Avesra exact audio-container owner v1\n[Unit]\nDescription=Avesra owned " + lane + " process\n"
            "After=network-online.target\nStartLimitIntervalSec=120\nStartLimitBurst=3\n[Service]\nType=simple\n"
            "ExecStart=/usr/bin/python3 " + str(root / "install-audio.py") + " run --manifest " + str(root / "owner.json") + " --sha256 " + identity + "\n"
            "Restart=on-failure\nRestartSec=10\nTimeoutStopSec=70\nKillMode=control-group\n"
            "MemoryMax=128M\nCPUQuota=50%\nNoNewPrivileges=yes\nUMask=0077\n[Install]\nWantedBy=default.target\n")
    unit.parent.mkdir(parents=True, exist_ok=True)
    with unit.open("x") as output:
        output.write(text)
    unit.chmod(0o600)
    if args.enable:
        command(["/usr/bin/systemctl", "--user", "daemon-reload"], limit=1024)
        command(["/usr/bin/systemctl", "--user", "enable", unit_name], limit=1024)
    print("Prepared", unit_name, "manifest", identity)
    print("No container started/stopped. Retire its existing idle owner explicitly before starting this unit.")
    print("Startup is", "enabled at user login" if args.enable else "demand-only; not enabled at login")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("prepare")
    for name in ("container", "image", "config", "socket"):
        create.add_argument("--" + name, required=True)
    create.add_argument("--deployment")
    create.add_argument("--enable", action="store_true")
    run = commands.add_parser("run")
    run.add_argument("--manifest", required=True)
    run.add_argument("--sha256", required=True)
    try:
        if sys.platform != "linux" or os.getuid() == 0 or sys.version_info < (3, 12):
            raise ValueError("Use Python3.12+ as the existing non-root audio service user")
        os.umask(0o077)
        arguments = parser.parse_args()
        supervise(arguments) if arguments.command == "run" else prepare(arguments)
    except Exception as error:
        print("Audio lifecycle refused:", error, file=sys.stderr)
        raise SystemExit(1)
