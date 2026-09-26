"""Mux timestamped isolated-WebView frames with the matching native diagnostic WAV.

Manual media preparation only; original PNG/WAV/JSON files remain untouched.
"""
import datetime
import json
from pathlib import Path
import subprocess
import sys

frames = Path(sys.argv[1]).resolve(strict=True)
wav = Path(sys.argv[2]).resolve(strict=True)
output = Path(sys.argv[3]).resolve()
capture = json.loads((frames / 'capture.json').read_text())
audio = json.loads(Path(str(wav) + '.json').read_text())
if (capture.get('kind') != 'sampled_production_settings_webview' or not capture.get('complete')
        or audio.get('state') != 'finalized' or not audio.get('hardware_silent')
        or audio.get('termination') not in ('stream_disposed', 'duration_limit')
        or not audio.get('frames')):
    raise SystemExit('Incomplete media; do not present as successful native evidence')
rows = capture['frames']
if not 1 <= len(rows) <= 600:
    raise SystemExit('Invalid frame inventory')
start = datetime.datetime.fromisoformat(capture['start_utc'].replace('Z', '+00:00')).timestamp()
end = datetime.datetime.fromisoformat(capture['end_utc'].replace('Z', '+00:00')).timestamp()
offset = int(audio['first_sample_utc_unix_ns']) / 1e9 - start
duration = end - start
if offset < 0 or offset + audio['duration_seconds'] > duration + 0.25:
    raise SystemExit('Video does not cover the full matching native audio interval')
timeline = frames / 'timeline.ffconcat'
with timeline.open('x', encoding='utf-8') as stream:
    stream.write('ffconcat version 1.0\n')
    for index, row in enumerate(rows):
        name = row['filename']
        if name != f'{index:05d}.png' or not (frames / name).is_file():
            raise SystemExit('Invalid frame path')
        next_time = rows[index + 1]['elapsed_ms'] / 1000 if index + 1 < len(rows) else duration
        span = next_time - row['elapsed_ms'] / 1000
        if not 0 < span <= 6:
            raise SystemExit('Invalid capture timing')
        stream.write(f"file '{name}'\nduration {span:.6f}\n")
    stream.write(f"file '{rows[-1]['filename']}'\n")
subprocess.run([
    'ffmpeg', '-nostdin', '-n', '-hide_banner', '-loglevel', 'error',
    '-f', 'concat', '-safe', '1', '-i', str(timeline), '-i', str(wav),
    '-filter_complex', f'[1:a]adelay={round(offset * 1000)}:all=1,apad[a]',
    '-map', '0:v:0', '-map', '[a]', '-vf', 'pad=ceil(iw/2)*2:ceil(ih/2)*2',
    '-r', '10', '-fps_mode', 'cfr', '-c:v', 'libx264', '-preset', 'veryfast',
    '-threads', '2', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-b:a', '192k',
    '-t', f'{duration:.6f}', '-movflags', '+faststart', str(output),
], check=True)
metadata = dict(scope='Sampled real production Settings WebView with synchronized native postmix diagnostic audio; physical output intentionally silent',
                frame_directory=str(frames), native_wav=str(wav), audio_offset_seconds=offset,
                duration_seconds=duration, video_fps=10, original_capture_interval_ms=capture['target_interval_ms'])
with Path(str(output) + '.json').open('x') as stream:
    json.dump(metadata, stream, indent=2)
print(json.dumps(dict(output=str(output), audio_offset_seconds=offset, duration_seconds=duration)))
