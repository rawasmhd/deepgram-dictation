"""Run dictate.py with a WAV file as the microphone, for the benchmark.

    pythonw bench/run_python.py bench/sample.wav [NAME=VALUE ...]

Every recording plays the file from the start, in real time, then silence.
NAME=VALUE overrides a setting at the top of dictate.py, for example
LIVE_PASTE=False or TRANSCRIBE_MODE='batch'. Nothing else changes.
"""

import ast
import sys
import threading
import time
import wave
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import dictate  # noqa: E402

BLOCK_FRAMES = 320          # 20 ms at 16 kHz, like a typical audio callback


def load_wav(path):
    with wave.open(str(path), "rb") as w:
        if (w.getframerate(), w.getnchannels(), w.getsampwidth()) != (
                dictate.SAMPLE_RATE, dictate.CHANNELS, dictate.SAMPLE_WIDTH):
            sys.exit(f"{path}: must be 16 kHz, mono, 16-bit")
        pcm = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16)
    return pcm.reshape(-1, 1)


class FileInputStream:
    """Stands in for sounddevice.InputStream."""

    pcm = None

    def __init__(self, samplerate, channels, dtype, callback, **_):
        self._callback = callback
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, daemon=True)

    def start(self):
        self._thread.start()

    def stop(self):
        self._stop.set()
        self._thread.join(timeout=1.0)

    def close(self):
        pass

    def _run(self):
        silence = np.zeros((BLOCK_FRAMES, 1), dtype=np.int16)
        start = time.perf_counter()
        n = 0
        while not self._stop.is_set():
            block = self.pcm[n * BLOCK_FRAMES:(n + 1) * BLOCK_FRAMES]
            if len(block) < BLOCK_FRAMES:
                block = np.concatenate([block, silence[:BLOCK_FRAMES - len(block)]])
            self._callback(block, BLOCK_FRAMES, None, None)
            n += 1
            # real time: wait until the next block is "recorded"
            delay = start + n * BLOCK_FRAMES / dictate.SAMPLE_RATE - time.perf_counter()
            if delay > 0:
                self._stop.wait(delay)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    for arg in sys.argv[2:]:
        name, value = arg.split("=", 1)
        if not hasattr(dictate, name):
            sys.exit(f"dictate.py has no setting {name}")
        setattr(dictate, name, ast.literal_eval(value))
    FileInputStream.pcm = load_wav(sys.argv[1])
    dictate.sd.InputStream = FileInputStream
    dictate.LOG_FILE = Path(__file__).resolve().parent / "bench.log"
    dictate.main()
