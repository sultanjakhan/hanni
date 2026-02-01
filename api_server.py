"""
Hanni API Server - Bridge between mobile app and MLX
Run: python api_server.py
"""
import asyncio
import json
import re
from http.server import HTTPServer, BaseHTTPRequestHandler
from socketserver import ThreadingMixIn
import httpx

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

    # Try to find Russian text
    for line in reasoning.split('\n'):
        if line.strip().startswith(('1.', '2.', '3.', '**', '*   ')):
            continue
        cyrillic = re.search(r'[А-Яа-яЁё][А-Яа-яЁё\s!?.,-]+', line)
        if cyrillic and len(cyrillic.group()) > 5:
            return cyrillic.group().strip()

    # Try Option format
    match = re.search(r'Option 1[^:]*:\*?\s*([^\n(]+)', reasoning)
    if match:
        return match.group(1).strip().rstrip('* ')

    return "Привет! Чем могу помочь?"


class Handler(BaseHTTPRequestHandler):
    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'POST, GET, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type')
        self.end_headers()

    def do_POST(self):
        if self.path == '/chat':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            data = json.loads(post_data.decode('utf-8'))

            message = data.get('message', '')

            # Call LLM
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

            # Send response
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            self.wfile.write(json.dumps({"response": text}).encode())

        else:
            self.send_response(404)
            self.end_headers()

    def do_GET(self):
        if self.path == '/health':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({"status": "ok"}).encode())
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        print(f"[API] {args[0]}")


class ThreadedHTTPServer(ThreadingMixIn, HTTPServer):
    allow_reuse_address = True


def main():
    port = 8080
    server = ThreadedHTTPServer(('0.0.0.0', port), Handler)
    print(f"=" * 50)
    print(f"Hanni API Server")
    print(f"Listening on 0.0.0.0:{port}")
    print(f"=" * 50)

    # Get local IP
    import socket
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect(('8.8.8.8', 80))
        ip = s.getsockname()[0]
        print(f"\nYour Mac IP: {ip}")
        print(f"Set this in the mobile app!")
    except:
        pass
    finally:
        s.close()

    print(f"\nWaiting for connections...")
    server.serve_forever()


if __name__ == "__main__":
    main()
