"""Local operator health/load client. It never reads or submits audio."""
import argparse
import json
import socket
import struct
import time
import uuid


def read_exact(connection, count):
    data = bytearray()
    while len(data) < count:
        chunk = connection.recv(count - len(data))
        if not chunk:
            raise RuntimeError("Audio service disconnected")
        data.extend(chunk)
    return data


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("operation", choices=("health", "load"))
    parser.add_argument("--socket", required=True)
    args = parser.parse_args()
    request = {"version": 1, "operation": args.operation}
    if args.operation == "load":
        now = int(time.time() * 1000)
        request.update(request_id=str(uuid.uuid4()), session_id=str(uuid.uuid4()),
                       capture_epoch=1, sequence=1, issued_at_ms=now, expires_at_ms=now + 120_000)
    body = json.dumps(request).encode()
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(125)
        connection.connect(args.socket)
        connection.sendall(struct.pack("!I", len(body)) + body)
        length = struct.unpack("!I", read_exact(connection, 4))[0]
        if not 0 < length <= 4096:
            raise RuntimeError("Invalid administration response")
        response = json.loads(read_exact(connection, length))
        allowed = {"version", "lane", "model_revision", "state", "streaming", "cancellation", "permission_authority", "busy", "successful_inferences", "last_inference_ms"}
        if not isinstance(response, dict):
            raise RuntimeError("Unexpected administration response")
        if "error" in response:
            if set(response) != {"error"} or not isinstance(response["error"], str) or len(response["error"]) > 64:
                raise RuntimeError("Invalid administration error")
            print("Audio operation failed.")
            raise SystemExit(1)
        if args.operation == "load":
            if response != {"state": "loaded_unqualified"}:
                raise RuntimeError("Model load was not acknowledged")
        else:
            if set(response) != allowed or type(response["version"]) is not int or response["version"] != 1:
                raise RuntimeError("Invalid health schema")
            if response["lane"] not in {"speaker", "asr", "tts", "voice-design"} or response["state"] not in {"unavailable", "loading", "loaded_unqualified", "termination_pending"}:
                raise RuntimeError("Invalid health state")
            revision = response["model_revision"]
            if not isinstance(revision, str) or len(revision) != 40 or any(c not in "0123456789abcdef" for c in revision):
                raise RuntimeError("Invalid model revision")
            if type(response["streaming"]) is not bool or (response["streaming"] and response["lane"] not in {"asr", "tts"}) or response["permission_authority"] is not False or type(response["busy"]) is not bool or response["cancellation"] != "terminate_process":
                raise RuntimeError("Unexpected service capabilities")
            if type(response["successful_inferences"]) is not int or response["successful_inferences"] < 0:
                raise RuntimeError("Invalid inference count")
            duration = response["last_inference_ms"]
            if duration is not None and (type(duration) not in (int, float) or not 0 <= duration <= 30_000):
                raise RuntimeError("Invalid inference duration")
        print(json.dumps(response))


if __name__ == "__main__":
    main()
