"""Model adapters; imports and GPU allocations occur only in the lane child."""
import base64
import math
from pathlib import Path

REVISIONS = {
    "speaker": "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286",
    "asr": "ebe59e5a817142986528bbbee5dba8db7b38ed50",
    "tts": "5d83992436eae1d760afd27aff78a71d676296fc",
    "voice-design": "5ecdb67327fd37bb2e042aab12ff7391903235d3",
}


def pcm(value):
    import numpy as np

    if not isinstance(value, str) or len(value) > 1_280_000:
        raise ValueError("invalid_audio")
    raw = base64.b64decode(value, validate=True)
    if not 320 <= len(raw) <= 960_000 or len(raw) % 2:
        raise ValueError("invalid_audio")
    return np.frombuffer(raw, dtype="<i2").astype(np.float32) / 32768.0


def text(value, limit):
    if not isinstance(value, str) or not value.strip() or len(value) > limit:
        raise ValueError("invalid_text")
    return value


def audio_result(waves, rate):
    import numpy as np

    if len(waves) != 1 or not 8000 <= rate <= 48000:
        raise ValueError("invalid_output")
    samples = np.asarray(waves[0], dtype=np.float32).reshape(-1)
    if not 0 < samples.size <= min(rate * 30, 720_000) or not np.isfinite(samples).all():
        raise ValueError("invalid_output")
    raw = (np.clip(samples, -1, 1) * 32767).astype("<i2").tobytes()
    return {"pcm_s16le": base64.b64encode(raw).decode("ascii"), "sample_rate": rate}


class Driver:
    def __init__(self, config):
        import torch

        self.torch = torch
        self.lane = config["lane"]
        self.prompt = None
        self.stream = None
        self.voices = None
        self.selected_voice = None
        if config.get("voice_store"):
            if self.lane not in {"tts", "voice-design"} or config.get("voice_preset"):
                raise ValueError("voice_store_configuration")
            from .voice_presets import Store
            self.voices = Store(config["voice_store"])
        model_path = Path(config["model_path"])
        if not model_path.is_dir() or config["model_revision"] != REVISIONS[self.lane]:
            raise ValueError("model_revision_unavailable")
        torch.set_num_threads(2)
        if not torch.cuda.is_available():
            raise ValueError("spark_gpu_unavailable")
        if self.lane == "speaker":
            from speechbrain.inference.speaker import EncoderClassifier

            self.model = EncoderClassifier.from_hparams(
                source=str(model_path), savedir=config["cache_path"],
                run_opts={"device": "cuda:0"},
                overrides={"pretrained_path": str(model_path)},
            )
        elif self.lane == "asr":
            from nemo.collections.asr.models import ASRModel

            files = list(model_path.glob("*.nemo"))
            if len(files) != 1:
                raise ValueError("model_artifact_unavailable")
            self.model = ASRModel.restore_from(str(files[0]), map_location="cuda")
            self.model = self.model.float()
            if config.get("asr_streaming", False):
                self.model.encoder.set_default_att_context_size([70, config.get("asr_right_context", 1)])
            self.model.eval()
        else:
            from qwen_tts import Qwen3TTSModel

            self.model = Qwen3TTSModel.from_pretrained(
                str(model_path), device_map="cuda:0", dtype=torch.bfloat16,
                attn_implementation="sdpa", local_files_only=True,
            )
            if self.lane == "tts" and config.get("voice_preset"):
                # Operator-selected generated asset, never arbitrary request paths.
                import json
                import soundfile as sf

                preset = Path(config["voice_preset"])
                metadata = json.loads(preset.with_suffix(".json").read_text())
                if metadata.get("base_revision") != REVISIONS["tts"] or metadata.get("kind") != "selected_generated_voice":
                    raise ValueError("voice_preset_incompatible")
                wave, rate = sf.read(preset, dtype="float32")
                if wave.ndim != 1 or not 8000 <= rate <= 48000 or not 0 < wave.size <= rate * 30:
                    raise ValueError("voice_preset_invalid")
                self.prompt = self.model.create_voice_clone_prompt(
                    ref_audio=(wave, rate), ref_text=text(metadata.get("text"), 2048),
                )
            if self.lane == "tts" and self.voices is not None:
                selected, revision = self.voices.selection()
                if selected is None and revision is not None:
                    raise ValueError("voice_selection_unreadable")
                if selected is not None:
                    self.prompt, self.selected_voice = self.prepare_voice(selected)

    def prepare_voice(self, selected):
        import numpy as np
        metadata, pcm = self.voices.candidate(selected)
        samples = np.frombuffer(pcm, dtype="<i2").astype(np.float32) / 32768.0
        try:
            prompt = self.model.create_voice_clone_prompt(ref_audio=(samples, 24000), ref_text=metadata["text"])
            return prompt, metadata["identity"]
        finally:
            pcm = samples = None

    def voice_request(self, operation, payload):
        if self.voices is None:
            raise ValueError("voice_store_unavailable")
        if operation == "create_voice":
            if self.lane != "voice-design" or set(payload) != {"text", "description"}:
                raise ValueError("voice_design_required")
            output = self.infer(payload)
            try:
                return self.voices.create(output, payload["text"], payload["description"])
            finally:
                output = None
        if self.lane != "tts":
            raise ValueError("tts_selection_required")
        if operation == "voice_status" and not payload:
            result = self.voices.status()
            result["active_voice"] = self.selected_voice
            result["active_state"] = "available" if self.selected_voice is not None and result["selection_state"] == "available" and result["selected"] == self.selected_voice else "unavailable"
            return result
        if operation == "select_voice" and set(payload) == {"identity", "selection_revision"}:
            prompt, selected = self.prepare_voice(payload["identity"])
            self.voices.select(selected, payload["selection_revision"])
            self.prompt, self.selected_voice = prompt, selected
            return {"selected": selected}
        if operation == "clear_voice" and set(payload) == {"selection_revision"}:
            self.voices.clear(payload["selection_revision"])
            self.prompt = self.selected_voice = None
            return {"selected": None}
        if operation == "discard_voice" and set(payload) == {"id", "revision"}:
            self.voices.discard(payload["id"], payload["revision"])
            return {"outcome": "discarded"}
        raise ValueError("invalid_voice_operation")

    def request(self, envelope, emit):
        if envelope["operation"] in {"create_voice", "voice_status", "select_voice", "clear_voice", "discard_voice"}:
            return self.voice_request(envelope["operation"], envelope["payload"])
        if self.lane == "tts" and self.voices is not None:
            selected, _ = self.voices.selection()
            if selected != self.selected_voice or selected is None:
                raise ValueError("voice_selection_changed")
            self.voices.candidate(selected)  # Revalidate durable identity/content before use.
        if envelope["operation"] == "tts_stream":
            if self.lane != "tts" or self.prompt is None or set(envelope["payload"]) != {"text"}:
                raise ValueError("selected_voice_required")
            from .streaming_tts import synthesize
            return synthesize(self.model, self.prompt, self.torch, text(envelope["payload"]["text"], 512), envelope["deadline"], emit)
        if envelope["operation"] != "stream":
            if self.stream is not None:
                raise ValueError("stream_active")
            return self.infer(envelope["payload"])
        if self.lane != "asr":
            raise ValueError("streaming_unsupported")
        from .streaming_asr import StreamingAsr
        import numpy as np

        payload = envelope["payload"]
        if set(payload) != {"pcm_s16le"} or not isinstance(payload["pcm_s16le"], str) or len(payload["pcm_s16le"]) > 8536:
            raise ValueError("invalid_stream_audio")
        raw = base64.b64decode(payload["pcm_s16le"], validate=True)
        if len(raw) > 6400 or len(raw) % 2:
            raise ValueError("invalid_stream_audio")
        samples = np.frombuffer(raw, dtype="<i2").astype(np.float32) / 32768.0
        raw = None
        if envelope["first"]:
            if self.stream is not None:
                raise ValueError("stream_active")
            self.stream = StreamingAsr(self.model, self.torch, envelope["deadline"])
        if self.stream is None:
            raise ValueError("stream_missing")
        try:
            return self.stream.push(samples, envelope["final"])
        finally:
            samples = None
            if self.stream.closed:
                self.stream = None

    def infer(self, payload):
        with self.torch.inference_mode():
            if self.lane == "speaker":
                if set(payload) != {"pcm_s16le"}:
                    raise ValueError("invalid_arguments")
                samples = pcm(payload["pcm_s16le"])
                if samples.size < 16000:
                    return {"outcome": "insufficient_speech"}
                tensor = self.torch.from_numpy(samples).unsqueeze(0).to("cuda")
                vector = self.model.encode_batch(tensor).reshape(-1).float().cpu().tolist()
                if len(vector) != 192 or not all(math.isfinite(v) for v in vector):
                    raise ValueError("invalid_embedding")
                # This representation is sensitive transient output, not an identity decision.
                return {"outcome": "embedding", "embedding": vector}
            if self.lane == "asr":
                if set(payload) != {"pcm_s16le"}:
                    raise ValueError("invalid_arguments")
                result = self.model.transcribe(audio=[pcm(payload["pcm_s16le"])], batch_size=1, verbose=False)
                if isinstance(result, tuple):
                    result = result[0]
                value = result[0] if isinstance(result[0], str) else result[0].text
                if not isinstance(value, str) or len(value) > 8192:
                    raise ValueError("invalid_transcript")
                return {"text": value, "final": True}
            if self.lane == "voice-design":
                if set(payload) != {"text", "description"}:
                    raise ValueError("invalid_arguments")
                waves, rate = self.model.generate_voice_design(
                    text=text(payload["text"], 512), language="English",
                    instruct=text(payload["description"], 1024), max_new_tokens=512,
                )
            else:
                if set(payload) != {"text"}:
                    raise ValueError("invalid_arguments")
                if self.prompt is None:
                    raise ValueError("voice_selection_required")
                waves, rate = self.model.generate_voice_clone(
                    text=text(payload["text"], 512), language="English",
                    voice_clone_prompt=self.prompt, max_new_tokens=512,
                )
            return audio_result(waves, rate)
