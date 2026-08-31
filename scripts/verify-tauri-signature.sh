#!/usr/bin/env bash
# Verify Tauri's outer-base64 .sig envelope with the committed Minisign key.
set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: verify-tauri-signature.sh <file> <file.sig> <updater.pub>" >&2
  exit 2
fi

MESSAGE="$1"
SIGNATURE="$2"
PUBLIC_KEY="$3"

for path in "$MESSAGE" "$SIGNATURE" "$PUBLIC_KEY"; do
  if [ ! -s "$path" ]; then
    echo "signature verification input is missing or empty: $path" >&2
    exit 1
  fi
done
if ! command -v minisign >/dev/null 2>&1; then
  echo "minisign is required for Tauri signature verification" >&2
  exit 1
fi

PYTHON=""
for candidate in python3 python; do
  if command -v "$candidate" >/dev/null 2>&1 \
     && "$candidate" -c 'import sys; raise SystemExit(sys.version_info < (3, 9))' \
          >/dev/null 2>&1; then
    PYTHON="$candidate"
    break
  fi
done
if [ -z "$PYTHON" ]; then
  echo "a working Python 3.9+ interpreter is required for signature verification" >&2
  exit 1
fi

DECODED="$(mktemp)"
trap 'rm -f "$DECODED"' EXIT
"$PYTHON" - "$SIGNATURE" "$DECODED" <<'PY'
import base64
import pathlib
import sys

source, destination = map(pathlib.Path, sys.argv[1:])
encoded = "".join(source.read_text(encoding="utf-8").split())
destination.write_bytes(base64.b64decode(encoded, validate=True))
PY

minisign -V -q -p "$PUBLIC_KEY" -m "$MESSAGE" -x "$DECODED"
