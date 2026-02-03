"""
Hanni API Server - Flask version
"""
import json
import re
import socket
import httpx
from flask import Flask, request, jsonify

app = Flask(__name__)

LLM_URL = "http://localhost:8000/v1/chat/completions"

SYSTEM_PROMPT = """Ты Hanni - персональный AI компаньон. Ты дружелюбный, заботливый и немного саркастичный.
Общайся на русском, будь кратким (2-3 предложения)."""


def extract_response(msg):
    """Extract actual response from GLM reasoning format"""
    content = msg.get("content", "")
    if content:
        return content

    reasoning = msg.get("reasoning", "")
    if not reasoning:
        return "Не могу ответить."

    for line in reasoning.split('\n'):
        if line.strip().startswith(('1.', '2.', '3.', '**', '*   ')):
            continue
        cyrillic = re.search(r'[А-Яа-яЁё][А-Яа-яЁё\s!?.,-]+', line)
        if cyrillic and len(cyrillic.group()) > 5:
            return cyrillic.group().strip()

    match = re.search(r'Option 1[^:]*:\*?\s*([^\n(]+)', reasoning)
    if match:
        return match.group(1).strip().rstrip('* ')

    return "Привет! Чем могу помочь?"


@app.after_request
def add_cors(response):
    response.headers['Access-Control-Allow-Origin'] = '*'
    response.headers['Access-Control-Allow-Methods'] = 'GET, POST, OPTIONS'
    response.headers['Access-Control-Allow-Headers'] = 'Content-Type'
    return response


@app.route('/health', methods=['GET'])
def health():
    return jsonify({"status": "ok"})


@app.route('/chat', methods=['POST', 'OPTIONS'])
def chat():
    if request.method == 'OPTIONS':
        return '', 200

    data = request.get_json()
    message = data.get('message', '')

    try:
        response = httpx.post(
            LLM_URL,
            json={
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": message}
                ],
                "temperature": 0.7,
                "max_tokens": 150,
            },
            timeout=120.0
        )

        if response.status_code == 200:
            result = response.json()
            msg = result["choices"][0]["message"]
            text = extract_response(msg)
        else:
            text = "LLM error"
    except Exception as e:
        text = f"Error: {str(e)}"

    return jsonify({"response": text})


if __name__ == "__main__":
    # Get local IP
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect(('8.8.8.8', 80))
        ip = s.getsockname()[0]
    except:
        ip = '0.0.0.0'
    finally:
        s.close()

    print("=" * 50)
    print("Hanni API Server (Flask)")
    print(f"Your Mac IP: {ip}")
    print("=" * 50)

    app.run(host='0.0.0.0', port=8080, debug=False, threaded=True)
