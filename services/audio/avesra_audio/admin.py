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
        allowed = {"version", "lane", "model_revision", "state", "streaming", "cancellation", "permission_authority", "busy", "successful_inferences", "last_inference_ms", "error"}
        if not isinstance(response, dict) or set(response) - allowed:
            raise RuntimeError("Unexpected administration response")
        print(json.dumps(response))
        if "error" in response:
            raise SystemExit(1)


if __name__ == "__main__":
    main()
