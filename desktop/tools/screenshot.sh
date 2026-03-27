#!/bin/bash
# Capture Hanni window screenshot
# Usage: ./screenshot.sh [output_path]
# Works with visible window via screencapture, falls back to html2canvas for minimized
OUT="${1:-/tmp/hanni_screenshot.png}"
TOKEN=$(cat ~/Library/Application\ Support/Hanni/api_token.txt 2>/dev/null)

# Check if Hanni is running
if ! osascript -e 'tell application "System Events" to name of process "hanni"' &>/dev/null; then
  echo "Hanni not running"; exit 1
fi

# Check if window is minimized
MINI=$(osascript -e 'tell application "System Events" to get value of attribute "AXMinimized" of first window of process "hanni"' 2>/dev/null)

if [ "$MINI" = "true" ]; then
  # Minimized — use html2canvas via HTTP API
  RESULT=$(curl -s --max-time 15 -X POST "http://127.0.0.1:8235/auto/eval" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"script":"var s=document.createElement(\"script\");s.src=\"https://cdnjs.cloudflare.com/ajax/libs/html2canvas/1.4.1/html2canvas.min.js\";document.head.appendChild(s);await new Promise(r=>s.onload=r);var c=await html2canvas(document.body,{scale:2});return c.toDataURL(\"image/png\")"}' 2>/dev/null)
  echo "$RESULT" | python3 -c "
import json,base64,sys
data=json.load(sys.stdin)
r=data.get('result','')
if isinstance(r,str) and r.startswith('data:'):
    b64=r.split(',',1)[1]
    with open('$OUT','wb') as f: f.write(base64.b64decode(b64))
    print('$OUT')
else:
    print('html2canvas failed',file=sys.stderr); sys.exit(1)
" || exit 1
else
  # Visible — use native screencapture (sharper)
  INFO=$(osascript -e 'tell application "System Events" to get {position, size} of first window of process "hanni"' 2>/dev/null)
  X=$(echo "$INFO" | cut -d, -f1 | tr -d ' ')
  Y=$(echo "$INFO" | cut -d, -f2 | tr -d ' ')
  W=$(echo "$INFO" | cut -d, -f3 | tr -d ' ')
  H=$(echo "$INFO" | cut -d, -f4 | tr -d ' ')
  screencapture -x -R${X},${Y},${W},${H} "$OUT" && echo "$OUT"
fi
