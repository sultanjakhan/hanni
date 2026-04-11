#!/bin/bash
# Capture Hanni window screenshot via html2canvas (works with hidden/background window)
# Usage: ./screenshot.sh [output_path] [port]
# Sends html2canvas source directly via win.eval() — bypasses CSP
OUT="${1:-/tmp/hanni_screenshot.png}"
PORT="${2:-8235}"
TOKEN=$(cat ~/Library/Application\ Support/Hanni/api_token.txt 2>/dev/null)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
H2C_PATH="$SCRIPT_DIR/../src/vendor/html2canvas.min.js"

if [ -z "$TOKEN" ]; then echo "No API token" >&2; exit 1; fi
if [ ! -f "$H2C_PATH" ]; then echo "html2canvas not found at $H2C_PATH" >&2; exit 1; fi

# Check API is reachable
if ! curl -s --max-time 2 -X POST "http://127.0.0.1:$PORT/auto/eval" \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"script":"return \"ok\""}' 2>/dev/null | grep -q '"ok"'; then
  echo "Hanni API not reachable on port $PORT" >&2; exit 1
fi

python3 - "$TOKEN" "$PORT" "$OUT" "$H2C_PATH" << 'PYEOF'
import urllib.request, json, base64, sys

token, port, out, h2c_path = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]

def eval_js(script):
    data = json.dumps({"script": script}).encode()
    req = urllib.request.Request(
        f"http://127.0.0.1:{port}/auto/eval",
        data=data,
        headers={"Content-Type": "application/json", "Authorization": f"Bearer {token}"}
    )
    resp = urllib.request.urlopen(req, timeout=30)
    return json.loads(resp.read().decode())

# Inject html2canvas if not already loaded (send full source via eval — bypasses CSP)
check = eval_js('return typeof html2canvas')
if check.get('result') != 'function':
    h2c_code = open(h2c_path, 'r').read()
    r = eval_js(h2c_code + '; return typeof html2canvas;')
    if r.get('result') != 'function':
        print(f"Inject failed: {r}", file=sys.stderr)
        sys.exit(1)

# Capture screenshot
r = eval_js("const c = await html2canvas(document.body, {scale:2, useCORS:true, logging:false}); return c.toDataURL('image/png');")
result = r.get('result', '')
if isinstance(result, str) and result.startswith('data:'):
    b64 = result.split(',', 1)[1]
    with open(out, 'wb') as f:
        f.write(base64.b64decode(b64))
    print(out)
else:
    print(f"Capture failed: {str(result)[:200]}", file=sys.stderr)
    sys.exit(1)
PYEOF
