"""Version-bound early codec decoding. This module never authorizes speech."""
import base64
import hashlib
from importlib import metadata
from pathlib import Path
import time

SOURCES = {
    "inference/qwen3_tts_model.py": "e1da450732857c1f5fe3e36ebab85db2f6dc6a48caaff6973f463384e30275e4",
    "core/models/modeling_qwen3_tts.py": "25c42656bcf810f06ef6bc1839bd7083f3c8cfedac3a147c4060b4262b1c96a0",
    "core/tokenizer_12hz/modeling_qwen3_tts_tokenizer_v2.py": "844e8dd8c0182ef9c6463c874631c22ef3c5a4fd1899dd657016164cc5379628",
}


def verify_package():
    package = metadata.distribution("qwen-tts")
    if package.version != "0.1.1":
        raise ValueError("streaming_tts_version")
    for name, expected in SOURCES.items():
        path = Path(package.locate_file("qwen_tts/" + name))
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise ValueError("streaming_tts_source")


def synthesize(wrapper, prompt, torch, text, deadline, emit):
    verify_package()
    if not isinstance(text, str) or not text.strip() or len(text) > 512:
        raise ValueError("invalid_text")
    if not isinstance(prompt, list) or len(prompt) != 1 or prompt[0].ref_code is None or not prompt[0].ref_text:
        raise ValueError("selected_generated_reference_required")
    model = wrapper.model
    tokenizer = model.speech_tokenizer
    if tokenizer.get_model_type() != "qwen3_tts_tokenizer_12hz" or tokenizer.get_output_sample_rate() != 24000 or tokenizer.get_decode_upsample_rate() != 1920:
        raise ValueError("streaming_tts_tokenizer")
    talker = model.talker
    groups = model.config.talker_config.num_code_groups
    eos = model.config.talker_config.codec_eos_token_id
    if type(groups) is not int or not 1 <= groups <= 32:
        raise ValueError("streaming_tts_geometry")
    voice = wrapper._prompt_items_to_voice_clone_prompt(prompt)
    reference = voice["ref_code"][0]
    if reference.ndim != 2 or reference.shape[1] != groups or not 0 < reference.shape[0] <= 375:
        raise ValueError("streaming_tts_reference")
    history = reference[-25:].detach().to(model.device)
    input_ids = wrapper._tokenize_texts([wrapper._build_assistant_text(text)])
    ref_ids = wrapper._tokenize_texts([wrapper._build_ref_text(prompt[0].ref_text)])
    complete = False
    sampled = False
    chunks = samples = 0

    def current():
        if time.monotonic() >= deadline:
            raise ValueError("streaming_tts_expired")

    def codec(_module, _inputs, output):
        nonlocal history, chunks, samples
        current()
        hidden = output.hidden_states
        if not isinstance(hidden, tuple) or len(hidden) != 2:
            raise ValueError("streaming_tts_output")
        code = hidden[-1]
        if code is None:  # Prefill carries no completed codec vector.
            return
        if code.shape != (1, groups):
            raise ValueError("streaming_tts_output")
        if int(code[0, 0].item()) == eos:
            return
        if chunks >= 375 or torch.any(code < 0).item():
            raise ValueError("streaming_tts_limit")
        code = code.detach()
        joined = torch.cat((history, code), dim=0)
        waves, rate = tokenizer.decode([{"audio_codes": joined}])
        if rate != 24000 or len(waves) != 1 or waves[0].ndim != 1 or waves[0].size != joined.shape[0] * 1920:
            raise ValueError("streaming_tts_audio")
        wave = waves[0][history.shape[0] * 1920:]
        import numpy as np
        if wave.size != 1920 or not np.isfinite(wave).all():
            raise ValueError("streaming_tts_audio")
        raw = (np.clip(wave, -1, 1) * 32767).astype("<i2").tobytes()
        chunks += 1
        samples += wave.size
        current()
        emit({"pcm_s16le": base64.b64encode(raw).decode("ascii"), "sample_rate": 24000,
              "sequence": chunks, "samples": int(wave.size)})
        history = joined[-25:].clone()

    original = talker.generate
    had_override = "generate" in talker.__dict__
    prior_override = talker.__dict__.get("generate")

    def generated(*args, **kwargs):
        nonlocal complete, sampled
        if sampled:
            raise ValueError("streaming_tts_generation_reentry")
        sampled = True
        result = original(*args, **kwargs)
        sequences = result.sequences
        if sequences.ndim != 2 or sequences.shape[0] != 1 or not 0 < sequences.shape[1] <= 376:
            raise ValueError("streaming_tts_tokens")
        complete = int(sequences[0, -1].item()) == eos
        current()
        return result

    hook = None
    installed = False
    try:
        hook = talker.register_forward_hook(codec)
        talker.generate = generated
        installed = True
        with torch.inference_mode():
            # Invoke prepared code generation directly. The wrapper's complete
            # waveform decode is intentionally not part of this early-audio path.
            result = model.generate(input_ids=input_ids, ref_ids=ref_ids,
                                    voice_clone_prompt=voice, languages=["English"],
                                    non_streaming_mode=False,
                                    **wrapper._merge_generate_kwargs(max_new_tokens=376))
            result = None
        if not sampled or chunks == 0:
            raise ValueError("streaming_tts_empty")
        return {"outcome": "complete" if complete else "truncated", "chunks": chunks,
                "samples": int(samples), "sample_rate": 24000}
    finally:
        try:
            if hook is not None:
                hook.remove()
        finally:
            if installed:
                if had_override:
                    talker.generate = prior_override
                else:
                    del talker.generate
            history = voice = input_ids = ref_ids = None
