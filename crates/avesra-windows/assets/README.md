# Galaxy ambience

The owner supplied `A Deep And Digital Background Ambient Galaxy With Slow Particle Effects.wav` on 2026-09-25 and requested its use as Avesra's background ambience. Its metadata identifies it as made with Suno. The source is 20 seconds, 48 kHz, stereo, signed 16-bit PCM. No repository-wide software license is asserted for this audio asset.

`galaxy-ambience.s16le` contains the source's PCM samples unchanged, with the WAV container and metadata removed. The original download is left intact. Runtime preparation applies rate conversion where needed and a one-second loop overlap; the voice-effects chain does not process this recording.

- Source WAV SHA-256: `821f0afbae5ebcef2457a9ddf8507328ab146a8ddb60962b03790dc28ee0c5c8`.
- Bundled PCM SHA-256: `7ab44decda7c68941f8bfc4a27286b954a061464602876dd783f0a6469da41b7`.

Import command (FFmpeg is needed only when replacing the asset, not by the app):

```powershell
ffmpeg -nostdin -i 'A Deep And Digital Background Ambient Galaxy With Slow Particle Effects.wav' -map 0:a:0 -map_metadata -1 -acodec pcm_s16le -f s16le galaxy-ambience.s16le
```

The native include has a compile-time byte-length contract of 3,840,000 bytes. Update preparation and provenance together if replacing the recording.
