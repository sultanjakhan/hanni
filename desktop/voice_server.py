#!/usr/bin/env python3
"""
Hanni Voice Server — Silero VAD + MLX Whisper STT + Silero TTS.

Modes:
  1. Call mode: GET /listen (SSE stream, continuous VAD + transcription)
  2. Single transcription: POST /transcribe (one utterance)
  3. TTS: POST /tts (local Silero TTS, no cloud)

Stack: Silero VAD (ONNX) + MLX Whisper large-v3 + Silero TTS v5 (all local, open-source)
"""

import json
import time
import threading
import queue
import io
import numpy as np
import sounddevice as sd
import torch

from loguru import logger

# ── Config ──
PORT = 8237
SAMPLE_RATE = 16000
VAD_CHUNK_SAMPLES = 512
SILENCE_TIMEOUT_MS = 500       # faster response (was 800)
MIN_SPEECH_MS = 400             # catch shorter utterances (was 500)
VAD_THRESHOLD = 0.6
WHISPER_MODEL = "mlx-community/whisper-large-v3-turbo"
MAX_TTS_TEXT_LENGTH = 2000      # prevent DoS via huge TTS requests
MAX_REQUEST_BODY = 10 * 1024    # 10KB max for any POST body
ALLOWED_ORIGIN = "tauri://localhost"  # only Tauri frontend

# ── Whisper hallucination filter ──
HALLUCINATIONS = {
    "спасибо за внимание", "спасибо за просмотр", "продолжение следует",
    "субтитры сделал", "субтитры подогнал", "редактор субтитров",
    "подписывайтесь на мой канал", "подписывайтесь на канал",
    "ставьте лайки", "не забудьте подписаться",
    "веселая музыка", "спокойная музыка", "грустная мелодия",
    "динамичная музыка", "торжественная музыка", "тревожная музыка",
    "музыкальная заставка", "аплодисменты", "смех",
    "перестрелка", "гудок поезда", "рёв мотора", "шум двигателя",
    "лай собак", "выстрелы", "стук в дверь",
    "thank you for watching", "thanks for watching", "thank you",
    "please subscribe", "subtitles by the amara",
    "transcription by castingwords", "the end", "bye bye", "bye",
    "satsang with mooji", "bbc radio",
}

def is_hallucination(text: str) -> bool:
    t = text.strip().lower()
    if len(t) < 2:
        return True
    # Exact match: filter only if the entire text is a hallucination phrase
    # (prevents false positives on "спасибо за просмотр презентации")
    if t in HALLUCINATIONS:
        return True
    # Repetition ratio check for garbled output
    if len(t) > 20:
        unique_chars = len(set(t))
        ratio = len(t) / max(unique_chars, 1)
        if ratio > 4.0:
            return True
    return False


# ── Silero VAD (for single transcription mode) ──
from silero_vad import load_silero_vad
logger.info("Loading Silero VAD...")
vad_model = load_silero_vad(onnx=True)
logger.info("Silero VAD loaded")

# ── Whisper (lazy load) ──
_whisper_loaded = False
def ensure_whisper():
    global _whisper_loaded
    if not _whisper_loaded:
        import mlx_whisper
        logger.info(f"Loading Whisper model: {WHISPER_MODEL}")
        mlx_whisper.transcribe(
            np.zeros(SAMPLE_RATE, dtype=np.float32),
            path_or_hf_repo=WHISPER_MODEL, language="ru",
        )
        _whisper_loaded = True
        logger.info("Whisper loaded (large-v3 full)")

def transcribe_audio(audio_float32: np.ndarray) -> str:
    import mlx_whisper
    if len(audio_float32) < SAMPLE_RATE * 0.3:
        return ""
    result = mlx_whisper.transcribe(
        audio_float32,
        path_or_hf_repo=WHISPER_MODEL,
        language="ru",
        temperature=(0.0,),
        condition_on_previous_text=False,
        no_speech_threshold=0.6,
        compression_ratio_threshold=2.4,
        hallucination_silence_threshold=1.5,
    )
    text = result.get("text", "").strip()
    if is_hallucination(text):
        return ""
    return text

def silero_speech_prob(audio_int16: np.ndarray) -> float:
    audio_float = audio_int16.astype(np.float32) / 32768.0
    tensor = torch.from_numpy(audio_float)
    with torch.no_grad():
        return vad_model(tensor, SAMPLE_RATE).item()


# ── Silero TTS (lazy load, multi-language) ──
TTS_SAMPLE_RATE = 48000
TTS_DEFAULT_SPEAKER = "xenia"
TTS_RU_SPEAKERS = ["aidar", "baya", "kseniya", "xenia", "eugene"]
TTS_EN_SPEAKERS = ["en_0", "en_21", "en_45", "en_56", "en_99",   # female (clear, natural)
                   "en_1", "en_7", "en_30", "en_72", "en_100"]    # male (clear, natural)
TTS_ALL_SPEAKERS = TTS_RU_SPEAKERS + TTS_EN_SPEAKERS

_tts_models = {}  # "ru" -> model, "en" -> model
_tts_lock = threading.Lock()

def _load_tts_model(lang: str):
    """Load Silero TTS model for language (ru or en)."""
    if lang in _tts_models:
        return
    with _tts_lock:
        if lang in _tts_models:
            return
        torch.set_num_threads(4)
        if lang == "en":
            logger.info("Loading Silero TTS v3 (English)...")
            model, _ = torch.hub.load(
                repo_or_dir='snakers4/silero-models',
                model='silero_tts',
                language='en',
                speaker='v3_en',
            )
        else:
            logger.info("Loading Silero TTS v5 (Russian)...")
            model, _ = torch.hub.load(
                repo_or_dir='snakers4/silero-models',
                model='silero_tts',
                language='ru',
                speaker='v5_ru',
            )
        model.to('cpu')
        _tts_models[lang] = model
        logger.info(f"Silero TTS [{lang}] loaded")

def _speaker_lang(speaker: str) -> str:
    """Determine language from speaker name."""
    return "en" if speaker.startswith("en_") else "ru"

def ensure_tts(speaker: str = TTS_DEFAULT_SPEAKER):
    lang = _speaker_lang(speaker)
    _load_tts_model(lang)

def synthesize_speech(text: str, speaker: str = TTS_DEFAULT_SPEAKER) -> bytes:
    """Generate WAV bytes from text using Silero TTS."""
    if speaker not in TTS_ALL_SPEAKERS:
        speaker = TTS_DEFAULT_SPEAKER
    lang = _speaker_lang(speaker)
    _load_tts_model(lang)
    model = _tts_models[lang]

    kwargs = dict(text=text, speaker=speaker, sample_rate=TTS_SAMPLE_RATE)
    if lang == "ru":
        kwargs["put_accent"] = True
        kwargs["put_yo"] = True
    audio = model.apply_tts(**kwargs)

    # Convert torch tensor to WAV bytes
    import wave
    buf = io.BytesIO()
    pcm = (audio.numpy() * 32767).astype(np.int16)
    with wave.open(buf, 'wb') as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)
        wf.setframerate(TTS_SAMPLE_RATE)
        wf.writeframes(pcm.tobytes())
    return buf.getvalue()


# ── Voice state (all mutable session state in one place) ──
class VoiceState:
    """Encapsulates all mutable voice session state instead of module-level globals."""
    def __init__(self):
        self.recording_active = threading.Event()
        self.call_active = threading.Event()
        self.call_paused = threading.Event()
        self.call_results = queue.Queue()
        self.transcribe_result = {"text": "", "error": ""}
        self.transcribe_ready = threading.Event()
        self.force_stop = threading.Event()

_state = VoiceState()

def record_and_transcribe(continuous=False):
    ensure_whisper()
    chunk_size = VAD_CHUNK_SAMPLES
    silence_chunks_needed = int(SILENCE_TIMEOUT_MS / (chunk_size / SAMPLE_RATE * 1000))
    min_speech_chunks = int(MIN_SPEECH_MS / (chunk_size / SAMPLE_RATE * 1000))

    try:
        stream = sd.InputStream(samplerate=SAMPLE_RATE, channels=1, dtype='int16', blocksize=chunk_size)
        stream.start()
    except Exception as e:
        err = f"Microphone error: {e}"
        logger.error(err)
        if continuous:
            _state.call_results.put({"error": err})
        else:
            _state.transcribe_result["error"] = err
            _state.transcribe_ready.set()
        return

    logger.info("Recording started (Silero VAD)")

    while True:
        speech_frames = []
        silence_count = 0
        speech_detected = False
        has_speech = False
        vad_model.reset_states()

        while not _state.force_stop.is_set():
            if continuous and not _state.call_active.is_set():
                break
            if not continuous and not _state.recording_active.is_set():
                break
            if continuous and _state.call_paused.is_set():
                try:
                    stream.read(chunk_size)
                except Exception as e:
                    logger.error(f"Stream read error during pause: {e}")
                    _state.call_results.put({"error": f"Microphone lost: {e}"})
                    _state.call_active.clear()
                    break
                if speech_detected:
                    speech_detected = False
                    speech_frames = []
                    silence_count = 0
                    vad_model.reset_states()
                continue

            try:
                data, _ = stream.read(chunk_size)
            except Exception:
                break

            prob = silero_speech_prob(data.flatten())
            if prob > VAD_THRESHOLD:
                speech_frames.append(data.copy())
                silence_count = 0
                if not speech_detected:
                    speech_detected = True
                    has_speech = True
                    logger.info(f"Speech detected (prob={prob:.2f})")
            else:
                if speech_detected:
                    speech_frames.append(data.copy())
                    silence_count += 1
                    if silence_count >= silence_chunks_needed:
                        logger.info("End of utterance")
                        break

        if _state.force_stop.is_set():
            _state.force_stop.clear()
            break

        if not has_speech or len(speech_frames) < min_speech_chunks:
            if continuous and _state.call_active.is_set():
                continue
            if not continuous:
                _state.transcribe_result["text"] = ""
                _state.transcribe_ready.set()
                break
            break

        audio_int16 = np.concatenate(speech_frames, axis=0).flatten()
        audio_float32 = audio_int16.astype(np.float32) / 32768.0
        duration = len(audio_float32) / SAMPLE_RATE
        logger.info(f"Transcribing {duration:.1f}s...")
        stt_t0 = time.time()
        text = transcribe_audio(audio_float32)
        stt_ms = int((time.time() - stt_t0) * 1000)
        logger.info(f"Result: '{text}' (STT {stt_ms}ms)")

        if continuous:
            if text:
                _state.call_results.put({"text": text, "stt_ms": stt_ms})
            if not _state.call_active.is_set():
                break
            continue
        else:
            _state.transcribe_result["text"] = text
            _state.transcribe_ready.set()
            break

    stream.stop()
    stream.close()
    logger.info("Recording stopped")


# ── HTTP + SSE Server (threaded so /listen SSE doesn't block /listen/pause etc.) ──
from http.server import HTTPServer, BaseHTTPRequestHandler
from socketserver import ThreadingMixIn

class ThreadedHTTPServer(ThreadingMixIn, HTTPServer):
    daemon_threads = True

class VoiceHandler(BaseHTTPRequestHandler):
    _listen_lock = threading.Lock()
    _listen_active = False

    def log_message(self, format, *args):
        pass

    def _origin(self):
        """Return allowed CORS origin — accept tauri:// and localhost."""
        origin = self.headers.get("Origin", "")
        if origin.startswith("tauri://") or origin.startswith("http://localhost") or origin.startswith("http://127.0.0.1"):
            return origin
        return ALLOWED_ORIGIN

    def _json(self, data, status=200):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", self._origin())
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _cors(self):
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", self._origin())
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def do_OPTIONS(self):
        self._cors()

    def _binary(self, data, content_type="audio/wav", status=200):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Access-Control-Allow-Origin", self._origin())
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _read_body(self):
        """Read POST body with size limit."""
        length = int(self.headers.get("Content-Length", 0))
        if length > MAX_REQUEST_BODY:
            return None  # caller should return 413
        return self.rfile.read(length) if length > 0 else b"{}"

    def do_GET(self):
        if self.path == "/health":
            self._json({"status": "ok", "model": WHISPER_MODEL, "vad": "silero", "tts": "silero_v5+v3_en"})
        elif self.path == "/tts/voices":
            self._json({
                "voices": TTS_ALL_SPEAKERS,
                "ru": TTS_RU_SPEAKERS,
                "en": TTS_EN_SPEAKERS,
                "default": TTS_DEFAULT_SPEAKER,
            })
        elif self.path == "/listen":
            # Prevent concurrent SSE sessions (mic can't be shared)
            with VoiceHandler._listen_lock:
                if VoiceHandler._listen_active:
                    self._json({"error": "Call mode already active"}, 409)
                    return
                VoiceHandler._listen_active = True
            try:
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Cache-Control", "no-cache")
                self.send_header("Access-Control-Allow-Origin", self._origin())
                self.end_headers()
                _state.call_active.set()
                _state.call_paused.clear()
                _state.force_stop.clear()
                t = threading.Thread(target=record_and_transcribe, args=(True,), daemon=True)
                t.start()
                try:
                    self.wfile.write(b"data: {\"phase\": \"listening\"}\n\n")
                    self.wfile.flush()
                    while _state.call_active.is_set():
                        try:
                            result = _state.call_results.get(timeout=0.5)
                            self.wfile.write(f"data: {json.dumps(result)}\n\n".encode())
                            self.wfile.flush()
                        except queue.Empty:
                            self.wfile.write(b": keepalive\n\n")
                            self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError):
                    pass
                finally:
                    _state.call_active.clear()
                    _state.force_stop.set()
            finally:
                with VoiceHandler._listen_lock:
                    VoiceHandler._listen_active = False
        else:
            self._json({"error": "not found"}, 404)

    def do_POST(self):
        if self.path == "/transcribe":
            _state.recording_active.set()
            _state.force_stop.clear()
            _state.transcribe_ready.clear()
            _state.transcribe_result["text"] = ""
            _state.transcribe_result["error"] = ""
            t = threading.Thread(target=record_and_transcribe, args=(False,), daemon=True)
            t.start()
            _state.transcribe_ready.wait(timeout=60)
            _state.recording_active.clear()
            if _state.transcribe_result["error"]:
                self._json({"error": _state.transcribe_result["error"]}, 500)
            else:
                self._json({"text": _state.transcribe_result["text"]})
        elif self.path == "/stop":
            _state.recording_active.clear()
            _state.force_stop.set()
            self._json({"status": "stopped"})
        elif self.path == "/finish":
            # Stop recording but let transcription happen (for press-and-hold)
            _state.recording_active.clear()
            self._json({"status": "finishing"})
        elif self.path == "/listen/pause":
            _state.call_paused.set()
            self._json({"status": "paused"})
        elif self.path == "/listen/resume":
            _state.call_paused.clear()
            self._json({"status": "resumed"})
        elif self.path == "/listen/stop":
            _state.call_active.clear()
            _state.call_paused.clear()
            _state.force_stop.set()
            while not _state.call_results.empty():
                try: _state.call_results.get_nowait()
                except queue.Empty: break
            self._json({"status": "stopped"})
        elif self.path == "/tts":
            try:
                raw = self._read_body()
                if raw is None:
                    self._json({"error": f"Request too large (max {MAX_REQUEST_BODY} bytes)"}, 413)
                    return
                body = json.loads(raw) if raw else {}
                text = body.get("text", "").strip()
                speaker = body.get("speaker", TTS_DEFAULT_SPEAKER)
                if not text:
                    self._json({"error": "no text"}, 400)
                    return
                if len(text) > MAX_TTS_TEXT_LENGTH:
                    text = text[:MAX_TTS_TEXT_LENGTH]
                logger.info(f"[TTS] speaker={speaker} len={len(text)}")
                wav_bytes = synthesize_speech(text, speaker)
                logger.info(f"[TTS] Done, {len(wav_bytes)} bytes")
                self._binary(wav_bytes)
            except Exception as e:
                logger.error(f"[TTS] Error: {e}")
                self._json({"error": str(e)}, 500)
        else:
            self._json({"error": "not found"}, 404)


def main():
    server = ThreadedHTTPServer(("127.0.0.1", PORT), VoiceHandler)
    logger.info(f"Hanni Voice Server on http://127.0.0.1:{PORT}")
    logger.info(f"STT: MLX Whisper ({WHISPER_MODEL})")
    logger.info(f"VAD: Silero VAD (ONNX, threshold={VAD_THRESHOLD})")
    logger.info(f"TTS: Silero v5 RU ({', '.join(TTS_RU_SPEAKERS)}) + v3 EN ({len(TTS_EN_SPEAKERS)} voices)")
    logger.info(f"Stack: 100% local open-source")

    # Preload models in background for zero cold-start latency
    def _preload():
        try:
            ensure_whisper()
            logger.info("Whisper preloaded in background")
        except Exception as e:
            logger.warning(f"Whisper preload failed: {e}")
        try:
            ensure_tts("xenia")
            logger.info("TTS (RU) preloaded in background")
        except Exception as e:
            logger.warning(f"TTS preload failed: {e}")
    threading.Thread(target=_preload, daemon=True).start()

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        logger.info("Shutting down")
        server.shutdown()


if __name__ == "__main__":
    main()
