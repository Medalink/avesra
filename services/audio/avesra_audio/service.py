"""Bounded same-UID Unix socket supervisor; one warm, cancellable lane process."""
import argparse
import asyncio
import base64
import contextlib
import json
import multiprocessing as mp
import os
from pathlib import Path
import socket
import signal
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
                    result = driver.request(payload, lambda chunk: pipe.send({"chunk": chunk}))
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
        self.lifecycle = asyncio.Lock()
        self.stream = None
        self.stream_timer = None
        self.streams_recent = {}

    def clear_stream(self):
        self.stream = None
        timer, self.stream_timer = self.stream_timer, None
        if timer is not None and timer is not asyncio.current_task():
            timer.cancel()

    def arm_stream(self, generation):
        if self.stream_timer is not None:
            self.stream_timer.cancel()
        async def expire():
            await asyncio.sleep(max(0, min(2, self.stream["deadline"] - time.monotonic())))
            await self.stop(generation)
        self.stream_timer = asyncio.create_task(expire())

    def current(self, generation, key, epoch, deadline):
        return generation == self.generation and self.sessions.get(key, (None,))[0] == epoch and time.monotonic() < deadline

    async def stop(self, expected_generation=None):
        async with self.lifecycle:
            if expected_generation is not None and expected_generation != self.generation:
                return self.state != "termination_pending"
            self.generation += 1
            self.clear_stream()
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
                    self.process = None
                    self.state = "unavailable"
            return self.process is None

    async def start(self, generation):
        async with self.lifecycle:
            if generation != self.generation:
                raise ValueError("cancelled")
            if self.state == "termination_pending" and self.process is not None:
                if self.process.is_alive():
                    raise ValueError("termination_pending")
                self.process.close()
                self.process = None
                self.state = "unavailable"
            if self.process is not None:
                return False
            self.state = "loading"
            context = mp.get_context("spawn")
            parent, worker = context.Pipe()
            self.pipe = parent
            self.process = context.Process(target=child, args=(worker, self.config), daemon=True)
            self.process.start()
            worker.close()
            return True

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

    async def dispatch(self, request, on_chunk=None):
        if not isinstance(request, dict) or type(request.get("version")) is not int or request["version"] != 1:
            return {"error": "invalid_request"}
        operation = request.get("operation")
        if operation == "health" and set(request) == {"version", "operation"}:
            return {
                "version": 1, "lane": self.config["lane"],
                "model_revision": self.config["model_revision"], "state": self.state,
                "streaming": self.config.get("asr_streaming", False) or self.config.get("tts_streaming", False), "cancellation": "terminate_process",
                "permission_authority": False, "busy": self.active is not None or self.stream is not None,
                "successful_inferences": self.successful_inferences,
                "last_inference_ms": self.last_inference_ms,
            }
        fields = {"version", "operation", "request_id", "session_id", "capture_epoch", "sequence", "issued_at_ms", "expires_at_ms"}
        voice_operations = {"create_voice", "voice_status", "select_voice", "clear_voice", "discard_voice"}
        if operation in {"infer", "stream", "tts_stream"} | voice_operations:
            fields |= {"payload"}
        if operation == "stream":
            fields |= {"chunk_sequence", "final"}
        if set(request) != fields or operation not in {"load", "infer", "cancel", "stream", "tts_stream"} | voice_operations:
            return {"error": "invalid_request"}
        if operation == "tts_stream" and (not self.config.get("tts_streaming", False) or on_chunk is None):
            return {"error": "streaming_unsupported"}
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
            if operation == "stream" and (not self.config.get("asr_streaming", False) or type(request["chunk_sequence"]) is not int or not 0 < request["chunk_sequence"] <= 3000 or type(request["final"]) is not bool):
                raise ValueError()
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
            active_matches = self.active is not None and self.active[:2] == (key, request["request_id"])
            stream_matches = self.stream is not None and (self.stream["session"], self.stream["id"]) == (key, request["request_id"])
            if not active_matches and not stream_matches:
                return {"error": "unknown_request"}
            stopped = await self.stop(self.generation)
            return {"outcome": "cancelled" if stopped else "termination_pending"}
        if self.stream is not None and operation == "stream" and (key, request["request_id"]) == (self.stream["session"], self.stream["id"]):
            expected = (self.stream["epoch"], self.stream["expires"], self.stream["next"])
            if self.active is not None or (request["capture_epoch"], request["expires_at_ms"], request["chunk_sequence"]) != expected:
                await self.stop(self.generation)
                return {"error": "stream_discontinuity"}
        if self.active is not None:
            return {"error": "busy"}
        if self.stream is not None and (operation != "stream" or (key, request["request_id"], request["capture_epoch"], request["expires_at_ms"], request.get("chunk_sequence")) != (self.stream["session"], self.stream["id"], self.stream["epoch"], self.stream["expires"], self.stream["next"])):
            return {"error": "stream_busy_or_stale"}
        first = operation == "stream" and self.stream is None
        if first and request["chunk_sequence"] != 1:
            return {"error": "stream_sequence"}
        if first or operation == "tts_stream":
            self.streams_recent = {k: until for k, until in self.streams_recent.items() if until > time.monotonic()}
            identity = (key, request["request_id"])
            if identity in self.streams_recent or len(self.streams_recent) >= 128:
                return {"error": "stream_replay_or_capacity"}
            if operation == "tts_stream":
                self.streams_recent[identity] = time.monotonic() + 31
        final = request.get("final", False)
        if operation == "stream":
            if self.state != "loaded_unqualified":
                return {"error": "load_required"}
            if first:
                self.streams_recent[identity] = time.monotonic() + 31
                self.stream = {"session": key, "id": request["request_id"], "epoch": request["capture_epoch"], "expires": request["expires_at_ms"], "deadline": deadline, "next": 1}
            deadline = self.stream["deadline"]
            self.arm_stream(self.generation)
        self.active = (key, request["request_id"], uuid.uuid4())
        owner = self.active
        epoch = request["capture_epoch"]
        payload = request.pop("payload", None)
        request = None
        generation = self.generation
        started = time.monotonic()
        try:
            if operation == "load":
                if await self.start(generation):
                    result = await self.receive(generation, deadline)
                    if not self.current(generation, key, epoch, deadline):
                        raise ValueError("stale_reply")
                    if "error" in result:
                        await self.stop(generation)
                        return result
                    self.state = "loaded_unqualified"
                return {"state": self.state}
            if self.state != "loaded_unqualified" or self.pipe is None:
                return {"error": "load_required"}
            if not isinstance(payload, dict):
                return {"error": "invalid_arguments"}
            # Off-loop send: OS pipe capacity must never block cancellation/health.
            envelope = {"operation": operation, "payload": payload, "first": first, "final": final, "deadline": deadline}
            await asyncio.wait_for(asyncio.to_thread(self.pipe.send, envelope), max(0.001, min(2, deadline - time.monotonic())))
            envelope = None
            payload = None
            result = await self.receive(generation, deadline)
            if not self.current(generation, key, epoch, deadline):
                raise ValueError("stale_reply")
            chunk_count = 0
            while "chunk" in result:
                chunk = result["chunk"]
                if operation != "tts_stream" or not isinstance(chunk, dict) or set(chunk) != {"pcm_s16le", "sample_rate", "sequence", "samples"}:
                    raise ValueError("invalid_stream_reply")
                if type(chunk["sequence"]) is not int or chunk["sequence"] != chunk_count + 1 or chunk_count >= 375 or type(chunk["sample_rate"]) is not int or chunk["sample_rate"] != 24000 or type(chunk["samples"]) is not int or chunk["samples"] != 1920:
                    raise ValueError("invalid_stream_reply")
                encoded = chunk["pcm_s16le"]
                if not isinstance(encoded, str) or len(encoded) != 5120 or len(base64.b64decode(encoded, validate=True)) != 3840:
                    raise ValueError("invalid_stream_reply")
                chunk_count += 1
                result.update(lane=self.config["lane"], model_revision=self.config["model_revision"], request_id=owner[1], session_id=key, capture_epoch=epoch)
                await asyncio.wait_for(on_chunk(result), max(0.001, min(0.5, deadline - time.monotonic())))
                chunk = encoded = result = None
                if not self.current(generation, key, epoch, deadline):
                    raise ValueError("stale_reply")
                result = await self.receive(generation, deadline)
                if not self.current(generation, key, epoch, deadline):
                    raise ValueError("stale_reply")
            if operation == "tts_stream" and "result" in result:
                terminal = result["result"]
                if not isinstance(terminal, dict) or set(terminal) != {"outcome", "chunks", "samples", "sample_rate"} or terminal["outcome"] not in {"complete", "truncated"} or type(terminal["chunks"]) is not int or terminal["chunks"] != chunk_count or chunk_count == 0 or type(terminal["samples"]) is not int or terminal["samples"] != chunk_count * 1920 or type(terminal["sample_rate"]) is not int or terminal["sample_rate"] != 24000:
                    raise ValueError("invalid_stream_terminal")
                result.update(request_id=owner[1], session_id=key, capture_epoch=epoch)
            if "result" in result:
                successful = terminal["outcome"] == "complete" if operation == "tts_stream" else operation in {"infer", "create_voice"} or operation == "stream" and final
                self.successful_inferences += int(successful)
                if successful or operation == "tts_stream":
                    self.last_inference_ms = round((time.monotonic() - started) * 1000, 3)
                result["lane"] = self.config["lane"]
                result["model_revision"] = self.config["model_revision"]
                if operation == "stream":
                    result["request_id"], result["session_id"], result["capture_epoch"] = owner[1], key, epoch
                    result["chunk_sequence"] = self.stream["next"]
                    self.stream["next"] += 1
                    if final:
                        self.clear_stream()
                    else:
                        self.arm_stream(generation)
            elif operation in {"stream", "tts_stream"}:
                await self.stop(generation)
            return result
        except (Exception, asyncio.CancelledError):
            if generation == self.generation:
                await self.stop(generation)
            return {"error": "cancelled_or_unavailable"}
        finally:
            payload = None
            envelope = None
            chunk = encoded = terminal = None
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
                async def on_chunk(value):
                    encoded = json.dumps(value, allow_nan=False, separators=(",", ":")).encode()
                    if len(encoded) > 16384:
                        raise ValueError("stream_output_too_large")
                    writer.write(struct.pack("!I", len(encoded)) + encoded)
                    await writer.drain()
                operation = asyncio.create_task(self.dispatch(request, on_chunk))
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
    if set(config) - {"lane", "model_path", "model_revision", "cache_path", "voice_preset", "voice_store", "asr_streaming", "asr_right_context", "tts_streaming"}:
        raise ValueError("Unknown configuration field")
    if config.get("lane") not in REVISIONS or config.get("model_revision") != REVISIONS[config["lane"]]:
        raise ValueError("Unsupported model revision")
    if type(config.get("asr_streaming", False)) is not bool or (config.get("asr_streaming", False) and config["lane"] != "asr"):
        raise ValueError("Invalid streaming configuration")
    if "asr_right_context" in config and (not config.get("asr_streaming", False) or type(config["asr_right_context"]) is not int or config["asr_right_context"] not in {0, 1, 6, 13}):
        raise ValueError("Invalid streaming context")
    if type(config.get("tts_streaming", False)) is not bool or (config.get("tts_streaming", False) and config["lane"] != "tts"):
        raise ValueError("Invalid TTS streaming configuration")
    path = Path(socket_path)
    parent = path.parent.stat()
    if parent.st_uid != os.getuid() or stat.S_IMODE(parent.st_mode) & 0o077:
        raise ValueError("Socket directory must belong to this user with mode 0700")
    if path.exists() or path.is_symlink():
        raise ValueError("Socket path exists; inspect previous service before removal")
    os.umask(0o077)
    service = Service(config)
    server = await asyncio.start_unix_server(service.connection, path=path, limit=MAX_PACKET + 4, backlog=8)
    task = asyncio.current_task()
    terminated = False

    def terminate():
        nonlocal terminated
        if not terminated:
            terminated = True
            task.cancel()

    loop = asyncio.get_running_loop()
    loop.add_signal_handler(signal.SIGTERM, terminate)
    try:
        async with server:
            await server.serve_forever()
    except asyncio.CancelledError:
        if not terminated:
            raise
    finally:
        await service.stop()
        path.unlink(missing_ok=True)
        loop.remove_signal_handler(signal.SIGTERM)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", required=True)
    parser.add_argument("--socket", required=True)
    args = parser.parse_args()
    asyncio.run(serve(args.config, args.socket))


if __name__ == "__main__":
    main()
