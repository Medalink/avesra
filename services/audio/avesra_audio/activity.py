"""Bounded tensor-only activity observations; no thresholds or identity claims."""
import hashlib
import importlib.metadata

REVISION = "cd03eee90fbec18297ac31b8c21546e596b7f71c"
ARTIFACT = "diar_streaming_sortformer_4spk-v2.1.nemo"
SHA256 = "8abd32832159c6ac1148c926b7276f35ba34582c444e559dce1f1253fea42ef8"


def load(directory):
    if importlib.metadata.version("nemo_toolkit") != "3.0.0":
        raise ValueError("activity_runtime_incompatible")
    from nemo.collections.asr.models import SortformerEncLabelModel

    path = directory / ARTIFACT
    if path.is_symlink() or not path.is_file() or path.stat().st_size != 471367680:
        raise ValueError("activity_artifact_unavailable")
    with path.open("rb") as source:
        if hashlib.file_digest(source, "sha256").hexdigest() != SHA256:
            raise ValueError("activity_artifact_changed")
    model = SortformerEncLabelModel.restore_from(str(path), map_location="cuda", strict=True)
    model.eval()
    features = model.preprocessor.featurizer
    modules = model.sortformer_modules
    if (not model.streaming_mode or features.sample_rate != 16000
            or features.hop_length != 160 or model.encoder.subsampling_factor != 8
            or modules.subsampling_factor != 8 or modules.n_spk != 4):
        raise ValueError("activity_geometry_incompatible")
    modules.chunk_len = 6
    modules.chunk_right_context = 7
    modules.fifo_len = 188
    modules.spkcache_update_period = 144
    modules.spkcache_len = 188
    modules._check_streaming_parameters()
    return model


def measure(model, torch, samples):
    if samples.ndim != 1 or not 16000 <= samples.size <= 160000:
        raise ValueError("activity_audio_bounds")
    expected = (samples.size + 1279) // 1280
    with torch.inference_mode():
        audio = torch.from_numpy(samples).unsqueeze(0).to(model.device)
        lengths = torch.tensor([samples.size], device=model.device, dtype=torch.long)
        scores = model.forward(audio_signal=audio, audio_signal_length=lengths)
        if (scores.ndim != 3 or scores.shape[0] != 1 or scores.shape[2] != 4
                or not expected <= scores.shape[1] <= expected + 1
                or not torch.isfinite(scores).all().item()
                or not ((scores >= 0) & (scores <= 1)).all().item()):
            raise ValueError("activity_output_invalid")
        return {"activity": {"model_revision": REVISION, "samples": int(samples.size),
                             "frames": scores[0, :expected].float().cpu().tolist()}}
