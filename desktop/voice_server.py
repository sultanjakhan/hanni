#!/usr/bin/env python3
"""
Hanni Voice Server — local voice activation with production-grade open-source libs.

Stack:
  - sounddevice (audio capture, PortAudio wrapper)
  - Silero VAD (ONNX, industry-standard voice activity detection)
  - mlx-whisper (Apple Silicon native Whisper large-v3)

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
import threading
import queue
import numpy as np
import sounddevice as sd
import torch
from silero_vad import load_silero_vad
from http.server import HTTPServer, BaseHTTPRequestHandler

# ── Config ──
PORT = 8237
SAMPLE_RATE = 16000
# Silero VAD requires 512-sample chunks at 16kHz (32ms windows)
VAD_CHUNK_SAMPLES = 512
SILENCE_TIMEOUT_MS = 800  # ms of silence after speech to end utterance
MIN_SPEECH_MS = 300  # minimum speech duration to process
VAD_THRESHOLD = 0.5  # Silero VAD speech probability threshold
WHISPER_MODEL = "mlx-community/whisper-large-v3-mlx"

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


# ── Silero VAD ──
print("[voice] Loading Silero VAD model...")
vad_model = load_silero_vad(onnx=True)
print("[voice] Silero VAD loaded")


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
        mlx_whisper.transcribe(
            np.zeros(SAMPLE_RATE, dtype=np.float32),
            path_or_hf_repo=WHISPER_MODEL,
            language="ru",
        )
        _whisper_loaded = True
        print("[voice] Whisper model loaded (large-v3 full)")

def transcribe_audio(audio_float32: np.ndarray) -> str:
    """Transcribe audio using mlx-whisper with max accuracy settings."""
    import mlx_whisper
    if len(audio_float32) < SAMPLE_RATE * 0.3:  # < 0.3s
        return ""
    result = mlx_whisper.transcribe(
        audio_float32,
        path_or_hf_repo=WHISPER_MODEL,
        language="ru",
        temperature=(0.0,),
        condition_on_previous_text=False,
        initial_prompt="Привет, как дела? Хорошо, понял. Давай посмотрим, что можно сделать.",
        no_speech_threshold=0.6,
        compression_ratio_threshold=2.4,
        hallucination_silence_threshold=2.0,
    )
    text = result.get("text", "").strip()
    if is_hallucination(text):
        return ""
    return text

def silero_vad_speech_prob(audio_int16: np.ndarray) -> float:
    """Get speech probability from Silero VAD for a 512-sample chunk."""
    audio_float = audio_int16.astype(np.float32) / 32768.0
    tensor = torch.from_numpy(audio_float)
    with torch.no_grad():
        prob = vad_model(tensor, SAMPLE_RATE).item()
    return prob

def record_and_transcribe(continuous=False):
    """
    Record audio, use Silero VAD to detect speech boundaries, transcribe.
    If continuous=True, keep listening and put results in call_mode_results queue.
    """
    ensure_whisper()

    chunk_size = VAD_CHUNK_SAMPLES  # 512 samples = 32ms
    silence_chunks_needed = int(SILENCE_TIMEOUT_MS / (chunk_size / SAMPLE_RATE * 1000))
    min_speech_chunks = int(MIN_SPEECH_MS / (chunk_size / SAMPLE_RATE * 1000))

    try:
        stream = sd.InputStream(
            samplerate=SAMPLE_RATE,
            channels=1,
            dtype='int16',
            blocksize=chunk_size,
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

    print("[voice] Recording started (Silero VAD)")

    while True:
        speech_frames = []
        silence_count = 0
        speech_detected = False
        has_speech = False
        # Reset Silero VAD state for new utterance
        vad_model.reset_states()

        # Listen for speech
        while not force_stop.is_set():
            if continuous and not call_mode_active.is_set():
                break
            if not continuous and not recording_active.is_set():
                break

            try:
                data, overflowed = stream.read(chunk_size)
            except Exception:
                break

            flat = data.flatten()
            prob = silero_vad_speech_prob(flat)
            is_speech = prob > VAD_THRESHOLD

            if is_speech:
                speech_frames.append(data.copy())
                silence_count = 0
                if not speech_detected:
                    speech_detected = True
                    has_speech = True
                    print(f"[voice] Speech detected (prob={prob:.2f})")
            else:
                if speech_detected:
                    speech_frames.append(data.copy())  # keep trailing silence
                    silence_count += 1
                    if silence_count >= silence_chunks_needed:
                        print(f"[voice] End of utterance (silence, {silence_count} chunks)")
                        break

        if force_stop.is_set():
            force_stop.clear()
            break

        if not has_speech or len(speech_frames) < min_speech_chunks:
            if continuous and call_mode_active.is_set():
                continue
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
            continue
        else:
            current_result["text"] = text
            result_ready.set()
            break

    stream.stop()
    stream.close()
    print("[voice] Recording stopped")


class VoiceHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass

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
            self._send_json({"status": "ok", "model": WHISPER_MODEL, "vad": "silero"})

        elif self.path == "/listen":
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()

            call_mode_active.set()
            force_stop.clear()

            t = threading.Thread(target=record_and_transcribe, args=(True,), daemon=True)
            t.start()

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
            recording_active.set()
            force_stop.clear()
            result_ready.clear()
            current_result["text"] = ""
            current_result["error"] = ""

            t = threading.Thread(target=record_and_transcribe, args=(False,), daemon=True)
            t.start()

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
            while not call_mode_results.empty():
                try:
                    call_mode_results.get_nowait()
                except queue.Empty:
                    break
            self._send_json({"status": "stopped"})

        else:
            self._send_json({"error": "not found"}, 404)


def main():
    threading.Thread(target=ensure_whisper, daemon=True).start()

    server = HTTPServer(("127.0.0.1", PORT), VoiceHandler)
    print(f"[voice] Hanni Voice Server running on http://127.0.0.1:{PORT}")
    print(f"[voice] Whisper: {WHISPER_MODEL}")
    print(f"[voice] VAD: Silero VAD (ONNX, threshold={VAD_THRESHOLD})")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[voice] Shutting down")
        server.shutdown()


if __name__ == "__main__":
    main()
