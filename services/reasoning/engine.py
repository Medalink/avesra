"""Fixed in-container observer/transport; reads one bounded stdin request.

No model imports, arbitrary URL, shell command or client-selected model options.
Executed only by the owned native adapter in an exact Docker container ID.
"""
import base64
import hashlib
import http.client
import importlib.metadata
import json
import os
from pathlib import Path
import re
import select
import stat
import subprocess
import sys
import time

SOURCES = {
    "entrypoints/openai/chat_completion/serving.py": "a2440b6b76ad87de86bcf6fc7061ad5d02ab67c1c3c89ca7de3eeb29aa12aa2f",
    "entrypoints/openai/chat_completion/protocol.py": "cb756e3d18e9061a2b306f305e10bd71d43f01ad1b236d0e9cbbb8756cd504dc",
    "entrypoints/serve/tokenize/serving.py": "3faa09f7824a068165769ee882158ac2caa9202b7f1905dc96b5d608aa9a762b",
    "v1/engine/async_llm.py": "bceed0b3f5f0c834fef79525f2462a092f082390f0070526280abc95945837dd",
    "v1/engine/output_processor.py": "c310a768931ab46ec9ac14ccbb433f07253b65553b7c859d1ca10f453efcb37c",
    "v1/core/sched/scheduler.py": "c710f49e41e974e5b7b8f1cd2f5fb9722523f4da0165252e16351199ebd03124",
    "v1/engine/core.py": "f4b1e07b6d91fac1549bc74a5c804e5a88793d663b5dc045c9e455db6550f5ad",
}
FAILURE = "engine_identity"


def read_bounded(path, limit):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as source:
        if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
            raise ValueError("non_regular_input")
        data = source.read(limit + 1)
    if len(data) > limit:
        raise ValueError("file_limit")
    return data


def quality_metadata(processes, started_ns, deadline):
    """Optional stable algorithm evidence. Never imports/initializes a model."""
    try:
        remaining = min(2, deadline - time.monotonic())
        if remaining <= 0:
            return None
        child = subprocess.Popen(["/usr/bin/nvidia-smi", "--query-gpu=uuid,name,driver_version,compute_cap",
                                  "--format=csv,noheader,nounits"], stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        output = bytearray()
        expires = time.monotonic() + remaining
        try:
            while True:
                wait = expires - time.monotonic()
                if wait <= 0 or not select.select([child.stdout], [], [], wait)[0]:
                    raise ValueError("gpu_metadata_expired")
                chunk = os.read(child.stdout.fileno(), 8193 - len(output))
                if not chunk:
                    break
                output.extend(chunk)
                if len(output) > 8192:
                    raise ValueError("gpu_metadata_limit")
            if child.wait(timeout=max(0.001, expires - time.monotonic())):
                raise ValueError("gpu_metadata_failed")
        finally:
            if child.poll() is None:
                child.kill()
            child.wait()
            child.stdout.close()
        rows = [[v.strip() for v in line.split(",")] for line in output.decode("ascii").splitlines()]
        if (len(rows) != 1 or len(rows[0]) != 4 or not rows[0][0].startswith("GPU-")
                or any(not v or "N/A" in v or len(v) > 256 for v in rows[0])):
            return None
        if (not re.fullmatch(r"GPU-[0-9a-fA-F-]{36}", rows[0][0])
                or not re.fullmatch(r"[0-9]+(?:\.[0-9]+)+", rows[0][2])
                or not re.fullmatch(r"[0-9]+\.[0-9]+", rows[0][3])):
            return None
        engine = next(pid for name, pid, _ in processes if name == "VLLM::EngineCor")
        maps = read_bounded(Path(f"/proc/{engine}/maps"), 1048576).decode()
        paths = {line.split()[-1] for line in maps.splitlines() if "/libcudart.so" in line}
        if len(paths) != 1:
            return None
        runtime = Path(next(iter(paths)))
        if (not runtime.is_absolute() or not str(runtime).startswith("/usr/local/lib/python3.12/dist-packages/nvidia/")
                or runtime.stat().st_ctime_ns > started_ns):
            return None
        runtime_hash = hashlib.sha256(read_bounded(runtime, 4194304)).hexdigest()
        versions = {}
        for name in ["nvidia-cuda-runtime", "torch", "vllm"]:
            distribution = importlib.metadata.distribution(name)
            version = distribution.version
            candidates = [f for f in distribution.files or [] if str(f).endswith(".dist-info/METADATA")]
            if not version or len(version) > 256 or len(candidates) != 1:
                return None
            metadata = Path(distribution.locate_file(candidates[0]))
            if metadata.stat().st_ctime_ns > started_ns:
                return None
            versions[name] = {"version": version, "metadata_sha256": hashlib.sha256(read_bounded(metadata, 1048576)).hexdigest()}
        if time.monotonic() >= deadline:
            return None
        descriptor = {"version": 1, "sources": SOURCES, "gpu": rows[0],
                      "runtime": {"path": str(runtime), "sha256": runtime_hash, "packages": versions}}
        return hashlib.sha256(json.dumps(descriptor, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    except Exception:
        return None


def run():
    global FAILURE
    raw = sys.stdin.buffer.read(65537)
    if len(raw) > 65536:
        raise ValueError("input_limit")
    request = json.loads(raw)
    selected = profile(request["profile"])
    if set(request) != {"profile", "mode", "model", "port", "key", "remaining_ms", "body", "expected", "started_ns"}:
        raise ValueError("request_shape")
    if request["mode"] not in {"inspect", "stream"} or request["model"] != selected["model"] or request["port"] != selected["port"]:
        raise ValueError("unsupported_deployment")
    if type(request["remaining_ms"]) is not int or not 1 <= request["remaining_ms"] <= 30000:
        raise ValueError("budget")
    deadline = time.monotonic() + request["remaining_ms"] / 1000
    root = Path("/usr/local/lib/python3.12/dist-packages/vllm")
    for name, expected in SOURCES.items():
        path = root / name
        if time.monotonic() >= deadline or path.stat().st_ctime_ns > request["started_ns"] or hashlib.sha256(read_bounded(path, 1048576)).hexdigest() != expected:
            raise ValueError("unreviewed_engine_source")
    if selected["guard"] is not None:
        guard = Path("/opt/avesra/bounded_start.py")
        if (guard.stat().st_ctime_ns > request["started_ns"]
                or hashlib.sha256(read_bounded(guard, 1048576)).hexdigest() != selected["guard"]):
            raise ValueError("unreviewed_startup_guard")
        limit = read_bounded(Path("/sys/fs/cgroup/memory.max"), 8192).decode().strip()
        swap = read_bounded(Path("/sys/fs/cgroup/memory.swap.max"), 8192).decode().strip()
        cpu = read_bounded(Path("/sys/fs/cgroup/cpu.max"), 8192).decode().split()
        if (not limit.isdecimal() or not 0 < int(limit) <= 36 * 1024**3 or swap != "0"
                or len(cpu) != 2 or not all(v.isdecimal() for v in cpu)
                or not 0 < int(cpu[0]) <= 2 * int(cpu[1])):
            raise ValueError("owned_resource_limits_changed")
    generation = json.loads(read_bounded(Path("/models/generation_config.json"), 65536))
    if (set(generation) != {"bos_token_id", "do_sample", "eos_token_id", "pad_token_id", "temperature", "top_k", "top_p"}
            or generation["eos_token_id"] != [248046, 248044]):
        raise ValueError("unreviewed_generation_defaults")
    processes = []
    descriptors = []
    for number, entry in enumerate(Path("/proc").iterdir()):
        if number >= 4096 or time.monotonic() >= deadline:
            raise ValueError("process_roster_limit")
        if not entry.name.isdigit():
            continue
        try:
            comm = read_bounded(entry / "comm", 256).decode().strip()
            if comm not in {"vllm", "VLLM::EngineCor"}:
                continue
            if len(descriptors) >= 2:
                raise ValueError("unsupported_process_roster")
            if comm == "vllm" and read_bounded(entry / "cmdline", 16384).decode().split("\0")[:-1] != ["/usr/bin/python3", "/usr/local/bin/vllm", "serve", *selected["command"]]:
                raise ValueError("engine_arguments_changed")
            before = read_bounded(entry / "stat", 16384).decode()
            fields = before[before.rfind(")") + 2:].split()
            fd = os.pidfd_open(int(entry.name))
            if read_bounded(entry / "stat", 16384).decode().split(")", 1)[1].split()[19] != fields[19]:
                raise ValueError("process_replaced")
            descriptors.append(fd)
            processes.append([comm, int(entry.name), fields[19]])
        except FileNotFoundError:
            raise ValueError("process_replaced")
    if sorted(row[0] for row in processes) != ["VLLM::EngineCor", "vllm"]:
        raise ValueError("unsupported_process_roster")
    identity = hashlib.sha256(json.dumps(sorted(processes), separators=(",", ":")).encode()).hexdigest()
    if request["expected"] is not None and identity != request["expected"]:
        raise ValueError("engine_changed")

    def current():
        remaining = deadline - time.monotonic()
        if remaining <= 0 or select.select(descriptors, [], [], 0)[0]:
            raise ValueError("engine_expired")
        return min(remaining, 20)

    def request_http(path, body=None):
        connection = http.client.HTTPConnection("127.0.0.1", selected["port"], timeout=current())
        headers = {"Authorization": "Bearer " + request["key"], "Content-Type": "application/json"}
        connection.request("GET" if body is None else "POST", path,
                           None if body is None else json.dumps(body), headers)
        response = connection.getresponse()
        current()
        if response.status != 200:
            raise ValueError("engine_unavailable")
        return connection, response

    connection, response = request_http("/v1/models")
    models = json.loads(response.read(65537))
    connection.close()
    current()
    entries = models.get("data")
    if not isinstance(entries, list) or len(entries) != 1:
        raise ValueError("model_roster")
    model = entries[0]
    if model.get("id") != request["model"] or model.get("root") != "/models" or model.get("max_model_len") != selected["capacity"]:
        raise ValueError("loaded_model_changed")
    body = request["body"]
    # The native adapter is the only producer; nevertheless reject option drift.
    if (body.get("model") != request["model"] or body.get("max_tokens") != 512
            or body.get("stop") != [] or body.get("stop_token_ids") != []
            or body.get("n") != 1 or body.get("tools") is not None):
        raise ValueError("request_options")
    FAILURE = "context_capacity"
    token_body = {"model": request["model"], "messages": body["messages"],
                  "add_generation_prompt": True, "add_special_tokens": False,
                  "chat_template_kwargs": body["chat_template_kwargs"]}
    connection, response = request_http("/tokenize", token_body)
    token_data = response.read(262145)
    connection.close()
    current()
    if len(token_data) > 262144:
        raise ValueError("tokenizer_limit")
    tokens = json.loads(token_data)
    count = tokens.get("count")
    if (type(count) is not int or count < 1 or tokens.get("max_model_len") != selected["capacity"]
            or not isinstance(tokens.get("tokens"), list) or len(tokens["tokens"]) != count
            or count + 512 > selected["capacity"]):
        raise ValueError("context_capacity")
    result = {"identity": identity, "prompt_tokens": count, "capacity": selected["capacity"],
              "quality": quality_metadata(processes, request["started_ns"], deadline), "stream": None}
    if request["mode"] == "stream":
        FAILURE = "stream_drain"
        connection, response = request_http("/v1/chat/completions", body)
        if response.getheader("Content-Type", "").split(";")[0].strip() != "text/event-stream":
            raise ValueError("stream_type")
        stream = bytearray()
        while True:
            if connection.sock is not None:
                connection.sock.settimeout(current())
            chunk = response.read1(65536)
            current()
            if not chunk:
                break
            if len(stream) + len(chunk) > 1048576:
                raise ValueError("stream_limit")
            stream.extend(chunk)
        connection.close()
        current()
        result["stream"] = base64.b64encode(stream).decode("ascii")
    current()
    print(json.dumps(result, separators=(",", ":")))


if __name__ == "__main__":
    try:
        run()
    except Exception:
        # Never include accepted text, token IDs, credentials or upstream bodies.
        print(json.dumps({"error": FAILURE}))
        raise SystemExit(1)
