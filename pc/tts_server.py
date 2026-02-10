"""
Qwen3-TTS Server for Hanni
Run on PC with NVIDIA GPU (12GB+).

Install:
  pip install torch torchvision --index-url https://download.pytorch.org/whl/cu128
  pip install transformers accelerate soundfile flask

Usage:
  python tts_server.py

The server listens on 0.0.0.0:8236 and accepts POST /tts with JSON:
  {"text": "Hello", "voice": "Qwen/Qwen3-TTS-0.6B", "lang": "ru"}

Returns audio/wav file.
"""

import io
import os
import sys
import torch
import soundfile as sf
from flask import Flask, request, send_file, jsonify
from transformers import AutoTokenizer, AutoModelForCausalLM

app = Flask(__name__)

MODEL_NAME = os.environ.get("TTS_MODEL", "Qwen/Qwen3-TTS-0.6B")
PORT = int(os.environ.get("TTS_PORT", "8236"))

print(f"Loading model {MODEL_NAME}...")
tokenizer = None
model = None


def load_model():
    global tokenizer, model
    try:
        from qwen3_tts import Qwen3TTS
        model = Qwen3TTS.from_pretrained(MODEL_NAME)
        model = model.to("cuda" if torch.cuda.is_available() else "cpu")
        print(f"Loaded Qwen3-TTS on {'CUDA' if torch.cuda.is_available() else 'CPU'}")
        return True
    except ImportError:
        # Fallback: try using transformers pipeline
        try:
            from transformers import pipeline
            model = pipeline("text-to-speech", model=MODEL_NAME, device=0 if torch.cuda.is_available() else -1)
            print(f"Loaded TTS pipeline on {'CUDA' if torch.cuda.is_available() else 'CPU'}")
            return True
        except Exception as e:
            print(f"Failed to load model: {e}")
            print("Falling back to edge-tts...")
            return False


USE_EDGE_FALLBACK = not load_model()


@app.route("/tts", methods=["POST"])
def tts():
    data = request.json or {}
    text = data.get("text", "")
    voice = data.get("voice", "ru-RU-SvetlanaNeural")

    if not text:
        return jsonify({"error": "no text"}), 400

    if USE_EDGE_FALLBACK:
        # Use edge-tts as fallback
        import asyncio
        import edge_tts

        async def generate():
            communicate = edge_tts.Communicate(text, voice)
            buf = io.BytesIO()
            async for chunk in communicate.stream():
                if chunk["type"] == "audio":
                    buf.write(chunk["data"])
            buf.seek(0)
            return buf

        try:
            buf = asyncio.run(generate())
            return send_file(buf, mimetype="audio/mp3", as_attachment=True, download_name="speech.mp3")
        except Exception as e:
            return jsonify({"error": str(e)}), 500
    else:
        try:
            if hasattr(model, "generate_speech"):
                # Qwen3-TTS API
                audio = model.generate_speech(text)
                buf = io.BytesIO()
                sf.write(buf, audio, 24000, format="WAV")
                buf.seek(0)
                return send_file(buf, mimetype="audio/wav", as_attachment=True, download_name="speech.wav")
            else:
                # Transformers pipeline
                result = model(text)
                buf = io.BytesIO()
                sf.write(buf, result["audio"], result["sampling_rate"], format="WAV")
                buf.seek(0)
                return send_file(buf, mimetype="audio/wav", as_attachment=True, download_name="speech.wav")
        except Exception as e:
            return jsonify({"error": str(e)}), 500


@app.route("/health", methods=["GET"])
def health():
    return jsonify({
        "status": "ok",
        "model": MODEL_NAME if not USE_EDGE_FALLBACK else "edge-tts (fallback)",
        "cuda": torch.cuda.is_available(),
        "gpu": torch.cuda.get_device_name(0) if torch.cuda.is_available() else None,
    })


if __name__ == "__main__":
    print(f"Qwen3-TTS server starting on port {PORT}...")
    app.run(host="0.0.0.0", port=PORT, debug=False)
