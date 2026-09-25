"""Bounded same-UID Unix socket supervisor; one warm, cancellable lane process."""
import argparse
import asyncio
import contextlib
import json
import multiprocessing as mp
import os
from pathlib import Path
import socket
import stat
import struct
import time
import uuid

MAX_PACKET = 2_000_000


def child(pipe, config):
    # Model libraries can print inputs on failure. The parent emits only fixed codes.
    with open(os.devnull, "w") as sink, contextlib.redirect_stdout(sink), contextlib.redirect_stderr(sink):
        os.dup2(sink.fileno(), 1)
        os.dup2(sink.fileno(), 2)
        try:
            from .drivers import Driver

            driver = Driver(config)
            pipe.send({"state": "loaded_unqualified"})
            while True:
                payload = pipe.recv()
                try:
                    result = driver.infer(payload)
                    payload = None
                    pipe.send({"result": result})
                except Exception:
                    pipe.send({"error": "inference_unavailable"})
                finally:
                    payload = None
                    result = None
        except Exception:
            with contextlib.suppress(Exception):
                pipe.send({"error": "runtime_incompatible"})
        finally:
            pipe.close()


class Service:
    def __init__(self, config):
        self.config = config
        self.process = None
        self.pipe = None
        self.state = "unavailable"
        self.active = None
        self.generation = 0
        self.connections = 0
        self.sessions = {}
        self.last_inference_ms = None
        self.successful_inferences = 0
        self.clock_wall = time.time()
        self.clock_mono = time.monotonic()

    def current(self, generation, key, epoch, deadline):
        return generation == self.generation and self.sessions.get(key, (None,))[0] == epoch and time.monotonic() < deadline

    async def stop(self):
        self.generation += 1
        process = self.process
        pipe, self.pipe = self.pipe, None
        self.state = "termination_pending" if process is not None else "unavailable"
        if pipe is not None:
            pipe.close()
        if process is not None:
            if process.is_alive():
                process.kill()
            await asyncio.to_thread(process.join, 2)
            if not process.is_alive():
                process.close()
                if self.process is process:
                    self.process = None
                    self.state = "unavailable"

    async def receive(self, generation, deadline):
        while time.monotonic() < deadline:
            if generation != self.generation or self.pipe is None:
                raise ValueError("cancelled")
            if self.pipe.poll():
                return await asyncio.wait_for(asyncio.to_thread(self.pipe.recv), max(0.01, deadline - time.monotonic()))
            if self.process is None or not self.process.is_alive():
                raise ValueError("runtime_unavailable")
            await asyncio.sleep(0.02)
        raise ValueError("deadline_exceeded")

    async def dispatch(self, request):
        if not isinstance(request, dict) or type(request.get("version")) is not int or request["version"] != 1:
            return {"error": "invalid_request"}
        operation = request.get("operation")
        if operation == "health" and set(request) == {"version", "operation"}:
            return {
                "version": 1, "lane": self.config["lane"],
                "model_revision": self.config["model_revision"], "state": self.state,
                "streaming": False, "cancellation": "terminate_process",
                "permission_authority": False, "busy": self.active is not None,
                "successful_inferences": self.successful_inferences,
                "last_inference_ms": self.last_inference_ms,
            }
        fields = {"version", "operation", "request_id", "session_id", "capture_epoch", "sequence", "issued_at_ms", "expires_at_ms"}
        if operation == "infer":
            fields |= {"payload"}
        if set(request) != fields or operation not in {"load", "infer", "cancel"}:
            return {"error": "invalid_request"}
        try:
            for name in ("request_id", "session_id"):
                value = request[name]
                if str(uuid.UUID(value)) != value or uuid.UUID(value).int == 0:
                    raise ValueError()
            for name in ("capture_epoch", "sequence", "issued_at_ms", "expires_at_ms"):
                if type(request[name]) is not int or not 0 < request[name] < 2**64:
                    raise ValueError()
            now = time.time() * 1000
            if abs(now - (self.clock_wall + time.monotonic() - self.clock_mono) * 1000) > 1000:
                raise ValueError()
            if not request["issued_at_ms"] <= now < request["expires_at_ms"] or request["expires_at_ms"] - request["issued_at_ms"] > (120_000 if operation == "load" else 30_000):
                raise ValueError()
            deadline = time.monotonic() + (request["expires_at_ms"] - now) / 1000
        except (ValueError, TypeError, AttributeError):
            return {"error": "invalid_request"}
        key = request["session_id"]
        # Retain sequence tombstones longer than every permitted request lifetime.
        monotonic = time.monotonic()
        self.sessions = {k: v for k, v in self.sessions.items() if monotonic - v[2] <= 125 or (self.active and self.active[0] == k)}
        previous = self.sessions.get(key)
        if previous and (request["capture_epoch"] < previous[0] or request["sequence"] <= previous[1]):
            return {"error": "stale_request"}
        if key not in self.sessions and len(self.sessions) >= 16:
            return {"error": "session_capacity"}
        self.sessions[key] = (request["capture_epoch"], request["sequence"], monotonic)
        if operation == "cancel":
            if self.active != (key, request["request_id"]):
                return {"error": "unknown_request"}
            await self.stop()
            return {"outcome": "cancelled"}
        if self.active is not None:
            return {"error": "busy"}
        self.active = (key, request["request_id"])
        owner = self.active
        epoch = request["capture_epoch"]
        payload = request.pop("payload", None)
        request = None
        generation = self.generation
        started = time.monotonic()
        try:
            if operation == "load":
                if self.state == "termination_pending" and self.process is not None:
                    if self.process.is_alive():
                        return {"error": "termination_pending"}
                    self.process.close()
                    self.process = None
                    self.state = "unavailable"
                if self.process is None:
                    self.state = "loading"
                    context = mp.get_context("spawn")
                    parent, worker = context.Pipe()
                    self.pipe = parent
                    self.process = context.Process(target=child, args=(worker, self.config), daemon=True)
                    self.process.start()
                    worker.close()
                    result = await self.receive(generation, deadline)
                    if not self.current(generation, key, epoch, deadline):
                        raise ValueError("stale_reply")
                    if "error" in result:
                        await self.stop()
                        return result
                    self.state = "loaded_unqualified"
                return {"state": self.state}
            if self.state != "loaded_unqualified" or self.pipe is None:
                return {"error": "load_required"}
            if not isinstance(payload, dict):
                return {"error": "invalid_arguments"}
            # Off-loop send: OS pipe capacity must never block cancellation/health.
            await asyncio.wait_for(asyncio.to_thread(self.pipe.send, payload), max(0.001, min(2, deadline - time.monotonic())))
            payload = None
            result = await self.receive(generation, deadline)
            if not self.current(generation, key, epoch, deadline):
                raise ValueError("stale_reply")
            if "result" in result:
                self.successful_inferences += 1
                self.last_inference_ms = round((time.monotonic() - started) * 1000, 3)
            return result
        except (Exception, asyncio.CancelledError):
            if generation == self.generation:
                await self.stop()
            return {"error": "cancelled_or_unavailable"}
        finally:
            payload = None
            if self.active == owner:
                self.active = None

    async def connection(self, reader, writer):
        if self.connections >= 8:
            writer.close()
            return
        self.connections += 1
        try:
            peer = writer.get_extra_info("socket").getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12)
            if struct.unpack("3i", peer)[1] != os.getuid():
                return
            async with asyncio.timeout(155):
                header = await asyncio.wait_for(reader.readexactly(4), 3)
                length = struct.unpack("!I", header)[0]
                if not 0 < length <= MAX_PACKET:
                    return
                body = await asyncio.wait_for(reader.readexactly(length), 3)
                request = json.loads(body)
                body = None
                operation = asyncio.create_task(self.dispatch(request))
                request = None
                reply = await operation
                operation = None
                encoded = json.dumps(reply, allow_nan=False, separators=(",", ":")).encode()
                reply = None
                if len(encoded) > MAX_PACKET:
                    encoded = b'{"error":"output_too_large"}'
                writer.write(struct.pack("!I", len(encoded)) + encoded)
                await asyncio.wait_for(writer.drain(), 3)
                encoded = None
        except (Exception, asyncio.CancelledError):
            pass
        finally:
            self.connections -= 1
            writer.close()
            with contextlib.suppress(Exception):
                await asyncio.wait_for(writer.wait_closed(), 1)


async def serve(config_path, socket_path):
    from .drivers import REVISIONS

    config = json.loads(Path(config_path).read_text())
    if set(config) - {"lane", "model_path", "model_revision", "cache_path", "voice_preset"}:
        raise ValueError("Unknown configuration field")
    if config.get("lane") not in REVISIONS or config.get("model_revision") != REVISIONS[config["lane"]]:
        raise ValueError("Unsupported model revision")
    path = Path(socket_path)
    parent = path.parent.stat()
    if parent.st_uid != os.getuid() or stat.S_IMODE(parent.st_mode) & 0o077:
        raise ValueError("Socket directory must belong to this user with mode 0700")
    if path.exists() or path.is_symlink():
        raise ValueError("Socket path exists; inspect previous service before removal")
    os.umask(0o077)
    service = Service(config)
    server = await asyncio.start_unix_server(service.connection, path=path, limit=MAX_PACKET + 4, backlog=8)
    try:
        async with server:
            await server.serve_forever()
    finally:
        await service.stop()
        path.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", required=True)
    parser.add_argument("--socket", required=True)
    args = parser.parse_args()
    asyncio.run(serve(args.config, args.socket))


if __name__ == "__main__":
    main()
