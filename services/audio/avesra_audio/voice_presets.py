"""Private generated-reference assets; paths never come from request payloads."""
import base64
import contextlib
import fcntl
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import time
import uuid
import wave

BASE = "5d83992436eae1d760afd27aff78a71d676296fc"
DESIGN = "5ecdb67327fd37bb2e042aab12ff7391903235d3"
STEM = re.compile(r"^[0-9a-f-]{36}-[0-9a-f-]{36}$")


def identity(value):
    if not isinstance(value, dict) or set(value) != {"id", "revision", "audio_sha256", "metadata_sha256"}:
        raise ValueError("invalid_voice_identity")
    for key in ("id", "revision"):
        if not isinstance(value[key], str) or str(uuid.UUID(value[key])) != value[key] or uuid.UUID(value[key]).int == 0:
            raise ValueError("invalid_voice_identity")
    for field in ("audio_sha256", "metadata_sha256"):
        digest = value[field]
        if not isinstance(digest, str) or len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
            raise ValueError("invalid_voice_identity")
    return value


def metadata_digest(metadata):
    value = dict(metadata)
    value["identity"] = dict(metadata["identity"])
    value["identity"].pop("metadata_sha256", None)
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, allow_nan=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()


class Store:
    def __init__(self, directory):
        self.directory = Path(directory)
        metadata = self.directory.lstat()
        if not self.directory.is_absolute() or not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != os.getuid() or stat.S_IMODE(metadata.st_mode) & 0o077 or self.directory.resolve() != self.directory:
            raise ValueError("voice_store_not_private")

    @contextlib.contextmanager
    def locked(self):
        fd = os.open(self.directory / ".writer.lock", os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
        try:
            metadata = os.fstat(fd)
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.getuid() or stat.S_IMODE(metadata.st_mode) & 0o077:
                raise ValueError("voice_lock_invalid")
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            yield
        finally:
            os.close(fd)

    def read(self, name, limit):
        fd = os.open(self.directory / name, os.O_RDONLY | os.O_NOFOLLOW)
        with os.fdopen(fd, "rb") as file:
            metadata = os.fstat(file.fileno())
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.getuid() or stat.S_IMODE(metadata.st_mode) & 0o077 or metadata.st_size > limit:
                raise ValueError("voice_record_invalid")
            result = file.read(limit + 1)
            if len(result) > limit:
                raise ValueError("voice_record_too_large")
            return result

    def publish(self, name, data, replace=False):
        temporary = self.directory / ("." + str(uuid.uuid4()) + ".tmp")
        try:
            fd = os.open(temporary, os.O_CREAT | os.O_EXCL | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, "wb") as file:
                file.write(data)
                file.flush()
                os.fsync(file.fileno())
            if replace:
                os.replace(temporary, self.directory / name)
            else:
                os.link(temporary, self.directory / name, follow_symlinks=False)
            fd = os.open(self.directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            try:
                os.fsync(fd)
            finally:
                os.close(fd)
        finally:
            temporary.unlink(missing_ok=True)

    def sync_directory(self):
        fd = os.open(self.directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)

    def entries(self):
        stems = set()
        for index, entry in enumerate(self.directory.iterdir()):
            if index >= 4096:
                raise ValueError("voice_directory_capacity")
            if entry.suffix not in {".json", ".wav"} or not STEM.fullmatch(entry.stem):
                continue
            first, second = entry.stem[:36], entry.stem[37:]
            identity({"id": first, "revision": second, "audio_sha256": "0" * 64, "metadata_sha256": "0" * 64})
            stems.add(entry.stem)
            if len(stems) > 32:
                raise ValueError("voice_candidate_capacity")
        return sorted(stems)

    def selection(self):
        try:
            raw = self.read("selected.json", 4096)
        except FileNotFoundError:
            return None, None
        digest = hashlib.sha256(raw).hexdigest()
        try:
            return identity(json.loads(raw)), digest
        except (ValueError, TypeError, KeyError):
            return None, digest

    def candidate(self, value):
        value = identity(value)
        stem = value["id"] + "-" + value["revision"]
        raw = self.read(stem + ".json", 8192)
        metadata = json.loads(raw)
        if set(metadata) != {"version", "identity", "base_revision", "design_revision", "kind", "text", "description", "created_at_ms", "sample_rate", "samples"} or metadata["version"] != 1 or type(metadata["version"]) is not int or metadata["identity"] != value or metadata["base_revision"] != BASE or metadata["design_revision"] != DESIGN or metadata["kind"] != "generated_voice_candidate":
            raise ValueError("voice_metadata_invalid")
        for field, limit in (("text", 512), ("description", 1024)):
            if not isinstance(metadata[field], str) or not metadata[field].strip() or len(metadata[field]) > limit:
                raise ValueError("voice_metadata_invalid")
        if type(metadata["created_at_ms"]) is not int or metadata["created_at_ms"] <= 0 or type(metadata["sample_rate"]) is not int or metadata["sample_rate"] != 24000 or type(metadata["samples"]) is not int or not 24000 <= metadata["samples"] <= 720000:
            raise ValueError("voice_metadata_invalid")
        if metadata_digest(metadata) != value["metadata_sha256"]:
            raise ValueError("voice_prompt_changed")
        audio = self.read(stem + ".wav", 1_440_044)
        if hashlib.sha256(audio).hexdigest() != value["audio_sha256"]:
            raise ValueError("voice_content_changed")
        with wave.open(io.BytesIO(audio), "rb") as wav:
            if wav.getnchannels() != 1 or wav.getsampwidth() != 2 or wav.getframerate() != 24000 or wav.getnframes() != metadata["samples"] or wav.getcomptype() != "NONE":
                raise ValueError("voice_audio_invalid")
            pcm = wav.readframes(metadata["samples"])
            if len(pcm) != metadata["samples"] * 2:
                raise ValueError("voice_audio_invalid")
        return metadata, pcm

    def create(self, output, text, description):
        if output.get("sample_rate") != 24000 or not isinstance(text, str) or not text.strip() or len(text) > 512 or not isinstance(description, str) or not description.strip() or len(description) > 1024:
            raise ValueError("voice_generation_invalid")
        encoded = output.get("pcm_s16le")
        if not isinstance(encoded, str) or len(encoded) > 1_920_000:
            raise ValueError("voice_generation_invalid")
        pcm = base64.b64decode(encoded, validate=True)
        if not 48000 <= len(pcm) <= 1_440_000 or len(pcm) % 2:
            raise ValueError("voice_generation_invalid")
        buffer = io.BytesIO()
        with wave.open(buffer, "wb") as wav:
            wav.setnchannels(1)
            wav.setsampwidth(2)
            wav.setframerate(24000)
            wav.writeframes(pcm)
        audio = buffer.getvalue()
        value = {"id": str(uuid.uuid4()), "revision": str(uuid.uuid4()), "audio_sha256": hashlib.sha256(audio).hexdigest()}
        metadata = {"version": 1, "identity": value, "base_revision": BASE, "design_revision": DESIGN, "kind": "generated_voice_candidate", "text": text, "description": description, "created_at_ms": int(time.time() * 1000), "sample_rate": 24000, "samples": len(pcm) // 2}
        value["metadata_sha256"] = metadata_digest(metadata)
        encoded_metadata = json.dumps(metadata, allow_nan=False, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded_metadata) > 8192:
            raise ValueError("voice_metadata_too_large")
        with self.locked():
            if len(self.entries()) >= 32:
                raise ValueError("voice_candidate_capacity")
            stem = value["id"] + "-" + value["revision"]
            self.publish(stem + ".wav", audio)
            self.publish(stem + ".json", encoded_metadata)
        return metadata

    def select(self, value, expected_revision):
        with self.locked():
            _, revision = self.selection()
            if revision != expected_revision:
                raise ValueError("voice_selection_changed")
            self.candidate(value)
            self.publish("selected.json", json.dumps(value, separators=(",", ":")).encode(), replace=True)

    def clear(self, expected_revision):
        with self.locked():
            _, revision = self.selection()
            if revision != expected_revision:
                raise ValueError("voice_selection_changed")
            (self.directory / "selected.json").unlink(missing_ok=True)
            self.sync_directory()

    def discard(self, candidate_id, revision):
        identity({"id": candidate_id, "revision": revision, "audio_sha256": "0" * 64, "metadata_sha256": "0" * 64})
        with self.locked():
            selected, file_revision = self.selection()
            if selected is None and file_revision is not None:
                raise ValueError("voice_selection_unreadable")
            if selected and selected["id"] == candidate_id and selected["revision"] == revision:
                raise ValueError("selected_voice_requires_clear")
            stem = candidate_id + "-" + revision
            (self.directory / (stem + ".json")).unlink(missing_ok=True)
            (self.directory / (stem + ".wav")).unlink(missing_ok=True)
            self.sync_directory()

    def status(self):
        with self.locked():
            selected, revision = self.selection()
            selection_state = "none" if revision is None else "unreadable" if selected is None else "unavailable"
            candidates = []
            for stem in self.entries():
                value = {"id": stem[:36], "revision": stem[37:]}
                try:
                    raw = json.loads(self.read(stem + ".json", 8192))
                    if raw["identity"]["id"] != value["id"] or raw["identity"]["revision"] != value["revision"]:
                        raise ValueError("voice_identity_mismatch")
                    metadata, pcm = self.candidate(raw["identity"])
                    pcm = None
                    if metadata["identity"]["id"] != value["id"] or metadata["identity"]["revision"] != value["revision"]:
                        raise ValueError("voice_identity_mismatch")
                    value.update(identity=metadata["identity"], description=metadata["description"], text=metadata["text"], created_at_ms=metadata["created_at_ms"], state="available")
                    if selected == metadata["identity"]:
                        selection_state = "available"
                except (OSError, ValueError, TypeError, KeyError, wave.Error):
                    value["state"] = "unavailable"
                candidates.append(value)
            return {"selected": selected, "selection_revision": revision, "selection_state": selection_state, "candidates": candidates}
