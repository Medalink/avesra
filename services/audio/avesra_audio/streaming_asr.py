"""Bounded live feature/cached decoder state; no file, logging or authority API."""
import time


class StreamingAsr:
    def __init__(self, model, torch, deadline):
        import numpy as np

        self.model, self.torch, self.np = model, torch, np
        self.deadline = min(deadline, time.monotonic() + 30)
        feature = model.preprocessor.featurizer
        if (feature.sample_rate, feature.n_fft, feature.hop_length,
                feature.win_length, feature.nfilt, feature.frame_splicing) != (16000, 512, 160, 400, 128, 1):
            raise ValueError("unsupported_streaming_frontend")
        if feature.exact_pad or feature.normalize not in (None, False, "NA"):
            raise ValueError("unsupported_streaming_frontend")
        if model.encoder.streaming_cfg is None:
            model.encoder.setup_streaming_params()
        self.config = model.encoder.streaming_cfg
        self.sampling = model.encoder.pre_encode.get_sampling_frames()
        pre = model.encoder.pre_encode
        if not pre.is_causal or pre._subsampling != "dw_striding" or pre.subsampling_factor != 8 or model.encoder.att_context_style != "chunked_limited":
            raise ValueError("unsupported_streaming_subsampling")
        self.raw = np.empty(0, dtype=np.float32)
        self.raw_start = self.total = self.emitted = 0
        self.features = torch.empty((1, 128, 0), device=model.device, dtype=torch.float32)
        self.left = self.features.clone()
        self.channel, self.temporal, self.length = model.encoder.get_initial_cache_state(batch_size=1)
        self.hypotheses = self.predictions = None
        self.step = 0
        self.closed = False
        self.text = ""
        for name in ("chunk_size", "shift_size", "pre_encode_cache_size"):
            for first in (True, False):
                value = self._size(getattr(self.config, name), first)
                if not 0 <= value <= 256 or (name != "pre_encode_cache_size" and value == 0):
                    raise ValueError("unsupported_streaming_geometry")
        if [self._size(self.sampling, first) for first in (True, False)] != [1, 8] or [self._size(self.config.pre_encode_cache_size, first) for first in (True, False)] != [0, 9] or type(self.config.drop_extra_pre_encoded) is not int or self.config.drop_extra_pre_encoded != 2:
            raise ValueError("unsupported_streaming_geometry")
        for first in (True, False):
            if self._size(self.config.chunk_size, first) != self._size(self.config.shift_size, first):
                raise ValueError("unsupported_streaming_geometry")

    @staticmethod
    def _size(value, first):
        if isinstance(value, (list, tuple)):
            if len(value) != 2:
                raise ValueError("unsupported_streaming_geometry")
            value = value[0 if first else 1]
        if type(value) is not int:
            raise ValueError("unsupported_streaming_geometry")
        return value

    def close(self):
        self.closed = True
        self.raw = self.features = self.left = None
        self.channel = self.temporal = self.length = None
        self.hypotheses = self.predictions = None
        self.text = ""

    def push(self, samples, final):
        if self.closed or time.monotonic() >= self.deadline:
            self.close()
            raise ValueError("stream_expired")
        if type(final) is not bool or samples.ndim != 1 or not 0 <= samples.size <= 3200:
            self.close()
            raise ValueError("invalid_stream_chunk")
        if (not final and samples.size == 0) or self.total + samples.size == 0 or not self.np.isfinite(samples).all() or self.total + samples.size > 480_000:
            self.close()
            raise ValueError("invalid_stream_chunk")
        try:
            with self.torch.inference_mode():
                self._features(samples, final)
                if final and self.emitted == 0:
                    raise ValueError("insufficient_final_features")
                self._decode(final)
                if time.monotonic() >= self.deadline:
                    raise ValueError("stream_expired")
                result = {"text": self.text, "final": final}
            if final:
                self.close()
            return result
        except Exception:
            self.close()
            raise

    def _features(self, samples, final):
        self.raw = self.np.concatenate((self.raw, samples))
        self.total += samples.size
        # Only stable STFT centers cross the live boundary. NeMo's valid final
        # feature count is floor(sample_count / hop), excluding its padded extra.
        end = self.total // 160 if final else max(0, (self.total - 256) // 160 + 1)
        if end > self.emitted:
            audio = self.torch.from_numpy(self.raw).unsqueeze(0).to(self.model.device)
            lengths = self.torch.tensor([self.raw.size], device=self.model.device, dtype=self.torch.long)
            features, _ = self.model.preprocessor(input_signal=audio, length=lengths)
            start = self.emitted - self.raw_start // 160
            stop = end - self.raw_start // 160
            fresh = features[:, :, start:stop].float()
            if fresh.shape != (1, 128, end - self.emitted) or not self.torch.isfinite(fresh).all().item():
                raise ValueError("invalid_stream_features")
            self.features = self.torch.cat((self.features, fresh), dim=-1)
            self.emitted = end
        retain_from = max(0, (self.emitted * 160 - 512) // 160 * 160)
        self.raw = self.raw[retain_from - self.raw_start:].copy()
        self.raw_start = retain_from
        if self.raw.size > 1024 or self.features.size(-1) > 512:
            raise ValueError("stream_buffer_capacity")

    def _decode(self, final):
        while self.features.size(-1):
            first = self.step == 0
            chunk = self._size(self.config.chunk_size, first)
            shift = self._size(self.config.shift_size, first)
            context = self._size(self.config.pre_encode_cache_size, first)
            available = self.features.size(-1)
            if shift > chunk:
                raise ValueError("unsupported_streaming_geometry")
            # Keep a complete last chunk until one more feature or explicit EOF
            # establishes whether keep_all_outputs must be set.
            if not final and available <= chunk:
                return
            take = min(chunk, available)
            if not final and take < self._size(self.sampling, first):
                raise ValueError("insufficient_final_features")
            # The pinned causal stride-2/kernel-3 layers each own left2/right1
            # padding. A final 1..7-frame suffix plus its nine context frames has
            # one real encoder output after dropping two cached outputs. Do not
            # copy the simulation buffer's sampling_frames early return and lose
            # that suffix, and do not append synthetic feature frames ourselves.
            output_frames = context + take
            for _ in range(3):
                output_frames = output_frames // 2 + 1
            if output_frames <= (0 if first else 2):
                raise ValueError("insufficient_final_features")
            history = self.left[:, :, -context:] if context else self.left[:, :, :0]
            if history.size(-1) < context:
                history = self.torch.nn.functional.pad(history, (context - history.size(-1), 0))
            current = self.torch.cat((history, self.features[:, :, :take]), dim=-1)
            last = final and available <= chunk
            (self.predictions, transcripts, self.channel, self.temporal,
             self.length, self.hypotheses) = self.model.conformer_stream_step(
                processed_signal=current.float(),
                processed_signal_length=self.torch.tensor([current.size(-1)], device=self.model.device, dtype=self.torch.long),
                cache_last_channel=self.channel, cache_last_time=self.temporal,
                cache_last_channel_len=self.length, previous_hypotheses=self.hypotheses,
                previous_pred_out=self.predictions, keep_all_outputs=last,
                drop_extra_pre_encoded=0 if first else self.config.drop_extra_pre_encoded,
                return_transcription=True,
            )
            if len(transcripts) != 1:
                raise ValueError("invalid_stream_transcript")
            value = transcripts[0] if isinstance(transcripts[0], str) else transcripts[0].text
            if not isinstance(value, str) or len(value) > 8192:
                raise ValueError("invalid_stream_transcript")
            self.text = value
            consumed = take if last else min(shift, take)
            self.left = self.torch.cat((self.left, self.features[:, :, :consumed]), dim=-1)[:, :, -256:].clone()
            self.features = self.features[:, :, consumed:].clone()
            self.step += 1
            if self.step > 3000 or time.monotonic() >= self.deadline:
                raise ValueError("stream_expired")
