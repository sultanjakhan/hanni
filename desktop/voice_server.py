#!/usr/bin/env python3
"""
Hanni Voice Server — lightweight local voice activation.

Stack:
  - sounddevice (audio capture)
  - RMS-based VAD (zero deps, adaptive noise floor)
  - mlx-whisper (Apple Silicon native Whisper)

Runs as HTTP server on port 8237. Tauri app communicates via HTTP requests.

Endpoints:
  POST /transcribe        — record → VAD detect end → transcribe → return text
  POST /stop              — force stop current recording
  GET  /health            — check if server is alive
  GET  /listen            — SSE stream for call mode (continuous listen → transcribe → emit)
  POST /listen/stop       — stop call mode listening
"""

import json
import sys
import time
import math
import threading
import queue
import numpy as np
import sounddevice as sd
from http.server import HTTPServer, BaseHTTPRequestHandler

# ── Config ──
PORT = 8237
SAMPLE_RATE = 16000
FRAME_MS = 30
FRAME_SAMPLES = int(SAMPLE_RATE * FRAME_MS / 1000)  # 480 samples
SILENCE_TIMEOUT_MS = 1000  # ms of silence to end utterance
MIN_SPEECH_MS = 300  # minimum speech duration to process
WHISPER_MODEL = "mlx-community/whisper-large-v3-turbo"

# ── RMS VAD config ──
RMS_SPEECH_THRESHOLD = 0.015  # absolute minimum RMS to consider speech
NOISE_FLOOR_ALPHA = 0.05  # smoothing factor for noise floor estimation
NOISE_FLOOR_MULTIPLIER = 3.0  # speech must be this many times above noise floor

# ── Whisper hallucination filter ──
HALLUCINATIONS = {
    # Russian
    "спасибо за внимание", "спасибо за просмотр", "продолжение следует",
    "субтитры сделал", "субтитры подогнал", "редактор субтитров",
    "подписывайтесь на мой канал", "подписывайтесь на канал",
    "ставьте лайки", "не забудьте подписаться",
    "веселая музыка", "спокойная музыка", "грустная мелодия",
    "динамичная музыка", "торжественная музыка", "тревожная музыка",
    "музыкальная заставка", "аплодисменты", "смех",
    "перестрелка", "гудок поезда", "рёв мотора", "шум двигателя",
    "лай собак", "выстрелы", "стук в дверь",
    # English
    "thank you for watching", "thanks for watching", "thank you",
    "please subscribe", "subtitles by the amara",
    "transcription by castingwords", "the end", "bye bye", "bye",
    "satsang with mooji", "bbc radio",
}

def is_hallucination(text: str) -> bool:
    t = text.strip().lower()
    if len(t) < 2:
        return True
    for h in HALLUCINATIONS:
        if h in t:
            return True
    # Detect repetitive text (looping hallucination)
    if len(t) > 20:
        unique_chars = len(set(t))
        ratio = len(t) / max(unique_chars, 1)
        if ratio > 4.0:
            return True
    return False

def compute_rms(audio_int16: np.ndarray) -> float:
    """Compute RMS of int16 audio, return as float in [0, 1] range."""
    float_data = audio_int16.astype(np.float32) / 32768.0
    return float(np.sqrt(np.mean(float_data ** 2)))

class AdaptiveVAD:
    """Simple RMS-based Voice Activity Detection with adaptive noise floor."""

    def __init__(self):
        self.noise_floor = 0.01  # initial estimate
        self.calibrated = False
        self.calibration_frames = []
        self.calibration_needed = 20  # ~600ms of calibration

    def is_speech(self, audio_int16: np.ndarray) -> bool:
        rms = compute_rms(audio_int16)

        # Calibration phase: collect noise floor samples
        if not self.calibrated:
            self.calibration_frames.append(rms)
            if len(self.calibration_frames) >= self.calibration_needed:
                self.noise_floor = np.median(self.calibration_frames) * 1.2
                self.noise_floor = max(self.noise_floor, 0.002)  # minimum floor
                self.calibrated = True
                print(f"[voice] Noise floor calibrated: {self.noise_floor:.4f}")
            return False

        # Slowly adapt noise floor during silence
        threshold = max(self.noise_floor * NOISE_FLOOR_MULTIPLIER, RMS_SPEECH_THRESHOLD)
        is_speech = rms > threshold

        if not is_speech:
            # Update noise floor slowly
            self.noise_floor = (1 - NOISE_FLOOR_ALPHA) * self.noise_floor + NOISE_FLOOR_ALPHA * rms
            self.noise_floor = max(self.noise_floor, 0.002)

        return is_speech

    def reset(self):
        """Reset for new recording session."""
        self.calibrated = False
        self.calibration_frames = []
        self.noise_floor = 0.01


# ── Globals ──
recording_active = threading.Event()
call_mode_active = threading.Event()
call_mode_results = queue.Queue()
current_result = {"text": "", "error": ""}
result_ready = threading.Event()
force_stop = threading.Event()

# Lazy-load whisper to avoid startup delay
_whisper_loaded = False
def ensure_whisper():
    global _whisper_loaded
    if not _whisper_loaded:
        import mlx_whisper
        print(f"[voice] Loading Whisper model: {WHISPER_MODEL}")
        mlx_whisper.transcribe(np.zeros(SAMPLE_RATE, dtype=np.float32), path_or_hf_repo=WHISPER_MODEL, language="ru")
        _whisper_loaded = True
        print("[voice] Whisper model loaded")

def transcribe_audio(audio_float32: np.ndarray) -> str:
    """Transcribe audio using mlx-whisper."""
    import mlx_whisper
    if len(audio_float32) < SAMPLE_RATE * 0.3:  # < 0.3s
        return ""
    result = mlx_whisper.transcribe(
        audio_float32,
        path_or_hf_repo=WHISPER_MODEL,
        language="ru",
        no_speech_threshold=0.6,
        compression_ratio_threshold=2.4,
    )
    text = result.get("text", "").strip()
    if is_hallucination(text):
        return ""
    return text

def record_and_transcribe(continuous=False):
    """
    Record audio, use VAD to detect speech boundaries, transcribe.
    If continuous=True, keep listening and put results in call_mode_results queue.
    """
    ensure_whisper()

    vad = AdaptiveVAD()
    frames_per_read = FRAME_SAMPLES
    silence_frames_needed = int(SILENCE_TIMEOUT_MS / FRAME_MS)
    min_speech_frames = int(MIN_SPEECH_MS / FRAME_MS)

    try:
        stream = sd.InputStream(
            samplerate=SAMPLE_RATE,
            channels=1,
            dtype='int16',
            blocksize=frames_per_read,
        )
        stream.start()
    except Exception as e:
        err = f"Microphone error: {e}"
        print(f"[voice] {err}")
        if continuous:
            call_mode_results.put({"error": err})
        else:
            current_result["error"] = err
            result_ready.set()
        return

    print("[voice] Recording started")

    while True:
        speech_frames = []
        silence_count = 0
        speech_detected = False
        has_speech = False
        vad.reset()

        # Listen for speech
        while not force_stop.is_set():
            if continuous and not call_mode_active.is_set():
                break
            if not continuous and not recording_active.is_set():
                break

            try:
                data, overflowed = stream.read(frames_per_read)
            except Exception:
                break

            is_speech = vad.is_speech(data.flatten())

            if is_speech:
                speech_frames.append(data.copy())
                silence_count = 0
                if not speech_detected:
                    speech_detected = True
                    has_speech = True
                    print("[voice] Speech detected")
            else:
                if speech_detected:
                    speech_frames.append(data.copy())  # keep trailing silence
                    silence_count += 1
                    if silence_count >= silence_frames_needed:
                        print("[voice] End of utterance (silence)")
                        break  # end of utterance

        if force_stop.is_set():
            force_stop.clear()
            break

        if not has_speech or len(speech_frames) < min_speech_frames:
            if continuous and call_mode_active.is_set():
                continue  # keep listening
            if not continuous:
                current_result["text"] = ""
                result_ready.set()
                break
            break

        # Convert to float32 for whisper
        audio_int16 = np.concatenate(speech_frames, axis=0).flatten()
        audio_float32 = audio_int16.astype(np.float32) / 32768.0

        duration = len(audio_float32) / SAMPLE_RATE
        print(f"[voice] Transcribing {duration:.1f}s of audio...")
        text = transcribe_audio(audio_float32)
        print(f"[voice] Result: '{text}'")

        if continuous:
            if text:
                call_mode_results.put({"text": text})
            if not call_mode_active.is_set():
                break
            continue  # keep listening
        else:
            current_result["text"] = text
            result_ready.set()
            break

    stream.stop()
    stream.close()
    print("[voice] Recording stopped")


class VoiceHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass  # suppress default logging

    def _send_json(self, data, status=200):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_cors(self):
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def do_OPTIONS(self):
        self._send_cors()

    def do_GET(self):
        if self.path == "/health":
            self._send_json({"status": "ok", "model": WHISPER_MODEL})

        elif self.path == "/listen":
            # SSE stream for call mode
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()

            call_mode_active.set()
            force_stop.clear()

            # Start recording thread
            t = threading.Thread(target=record_and_transcribe, args=(True,), daemon=True)
            t.start()

            # Stream results as SSE
            try:
                self.wfile.write(b"data: {\"phase\": \"listening\"}\n\n")
                self.wfile.flush()
                while call_mode_active.is_set():
                    try:
                        result = call_mode_results.get(timeout=0.5)
                        if "error" in result and result["error"]:
                            self.wfile.write(f"data: {json.dumps(result)}\n\n".encode())
                            self.wfile.flush()
                            break
                        if "text" in result and result["text"]:
                            self.wfile.write(f"data: {json.dumps(result)}\n\n".encode())
                            self.wfile.flush()
                    except queue.Empty:
                        # Send keepalive
                        self.wfile.write(b": keepalive\n\n")
                        self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                pass
            finally:
                call_mode_active.clear()
                force_stop.set()

        else:
            self._send_json({"error": "not found"}, 404)

    def do_POST(self):
        if self.path == "/transcribe":
            # Single recording: record → VAD → transcribe → return
            recording_active.set()
            force_stop.clear()
            result_ready.clear()
            current_result["text"] = ""
            current_result["error"] = ""

            t = threading.Thread(target=record_and_transcribe, args=(False,), daemon=True)
            t.start()

            # Wait for result (max 60s)
            result_ready.wait(timeout=60)
            recording_active.clear()

            if current_result["error"]:
                self._send_json({"error": current_result["error"]}, 500)
            else:
                self._send_json({"text": current_result["text"]})

        elif self.path == "/stop":
            recording_active.clear()
            force_stop.set()
            self._send_json({"status": "stopped"})

        elif self.path == "/listen/stop":
            call_mode_active.clear()
            force_stop.set()
            # Drain queue
            while not call_mode_results.empty():
                try:
                    call_mode_results.get_nowait()
                except queue.Empty:
                    break
            self._send_json({"status": "stopped"})

        else:
            self._send_json({"error": "not found"}, 404)


def main():
    # Pre-load whisper model in background
    threading.Thread(target=ensure_whisper, daemon=True).start()

    server = HTTPServer(("127.0.0.1", PORT), VoiceHandler)
    print(f"[voice] Hanni Voice Server running on http://127.0.0.1:{PORT}")
    print(f"[voice] Model: {WHISPER_MODEL}")
    print(f"[voice] VAD: adaptive RMS-based (no external deps)")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[voice] Shutting down")
        server.shutdown()


if __name__ == "__main__":
    main()
