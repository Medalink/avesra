"""Original-lifetime Sortformer state with bounded PCM/features and immutable scores."""
import time
from .activity import REVISION


class StreamingActivity:
    def __init__(self, model, torch, deadline):
        import numpy as np
        self.model, self.torch, self.np = model, torch, np
        self.deadline = min(deadline, time.monotonic() + 30)
        f = model.preprocessor.featurizer
        if ((f.sample_rate, f.n_fft, f.hop_length, f.win_length, f.nfilt, f.frame_splicing)
                != (16000, 512, 160, 400, 128, 1) or f.exact_pad
                or f.normalize not in (None, False, "NA") or model.async_streaming):
            raise ValueError("activity_stream_frontend")
        m = model.sortformer_modules
        if (m.chunk_len, m.chunk_right_context, m.chunk_left_context, m.subsampling_factor) != (6, 7, 1, 8):
            raise ValueError("activity_stream_geometry")
        self.state = m.init_streaming_state(batch_size=1, async_streaming=False, device=model.device)
        self.raw = np.empty(0, dtype=np.float32)
        self.features = torch.empty((1, 128, 0), device=model.device)
        self.left = self.features.clone()
        self.raw_start = self.total = self.emitted = self.output_frames = 0
        self.closed = False

    def close(self):
        self.closed = True
        self.raw = self.features = self.left = self.state = None

    def push(self, samples, final):
        if (self.closed or time.monotonic() >= self.deadline or type(final) is not bool
                or samples.ndim != 1 or not 0 < samples.size <= 3200 or samples.size % 160
                or self.total + samples.size > 160000 or not self.np.isfinite(samples).all()):
            self.close()
            raise ValueError("activity_stream_bounds")
        try:
            offset = self.output_frames
            output = []
            with self.torch.inference_mode():
                self._features(samples, final)
                while self.features.size(-1):
                    available = self.features.size(-1)
                    if not final and available < 104:
                        break
                    take = min(48, available)
                    right = min(56, available - take)
                    left = self.left.size(-1)
                    features = self.torch.cat((self.left, self.features[:, :, :take + right]), dim=-1)
                    empty = self.torch.empty((1, 0, 4), device=self.model.device)
                    self.state, scores = self.model.forward_streaming_step(
                        processed_signal=features.transpose(1, 2),
                        processed_signal_length=self.torch.tensor([features.size(-1)], device=self.model.device, dtype=self.torch.long),
                        streaming_state=self.state, total_preds=empty,
                        left_offset=left, right_offset=right,
                    )
                    expected = (take + 7) // 8
                    if (scores.shape != (1, expected, 4) or not self.torch.isfinite(scores).all().item()
                            or not ((scores >= 0) & (scores <= 1)).all().item()):
                        raise ValueError("activity_stream_output")
                    output.extend(scores[0].float().cpu().tolist())
                    self.output_frames += expected
                    self.left = self.torch.cat((self.left, self.features[:, :, :take]), dim=-1)[:, :, -8:].clone()
                    self.features = self.features[:, :, take:].clone()
                    if time.monotonic() >= self.deadline:
                        raise ValueError("activity_stream_expired")
            if len(output) > 16 or (final and self.output_frames != (self.total + 1279) // 1280):
                raise ValueError("activity_stream_incomplete")
            result = {"model_revision": REVISION, "samples": int(self.total), "frame_offset": offset,
                      "frames": output, "final": final}
            if final:
                self.close()
            return result
        except Exception:
            self.close()
            raise

    def _features(self, samples, final):
        self.raw = self.np.concatenate((self.raw, samples))
        self.total += samples.size
        end = self.total // 160 if final else max(0, (self.total - 256) // 160 + 1)
        if end > self.emitted:
            audio = self.torch.from_numpy(self.raw).unsqueeze(0).to(self.model.device)
            lengths = self.torch.tensor([self.raw.size], device=self.model.device, dtype=self.torch.long)
            features, _ = self.model.preprocessor(input_signal=audio, length=lengths)
            start, stop = self.emitted - self.raw_start // 160, end - self.raw_start // 160
            fresh = features[:, :, start:stop].float()
            if fresh.shape != (1, 128, end - self.emitted) or not self.torch.isfinite(fresh).all().item():
                raise ValueError("activity_stream_features")
            self.features = self.torch.cat((self.features, fresh), dim=-1)
            self.emitted = end
        retain = max(0, (self.emitted * 160 - 512) // 160 * 160)
        self.raw = self.raw[retain - self.raw_start:].copy()
        self.raw_start = retain
        if self.raw.size > 1024 or self.features.size(-1) > 128:
            raise ValueError("activity_stream_capacity")
