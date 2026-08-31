#!/usr/bin/env bash
# Dispatch a signed web-only update from the sole integration branch.
# Packaging, production-key access and immutable release upload happen only in
# the protected GitHub workflow; this local entrypoint never publishes bytes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

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
  echo "REFUSE: a working Python 3.9+ interpreter is required" >&2
  exit 1
fi

WEB_VERSION="${1:-}"
if [ -z "$WEB_VERSION" ]; then
  echo "usage: scripts/publish-web-update.sh <A.B.C.N>" >&2
  exit 2
fi

if ! command -v minisign >/dev/null 2>&1; then
  echo "REFUSE: minisign is required to verify the published signature" >&2
  echo "Install it first (macOS: brew install minisign)." >&2
  exit 1
fi

if [ "$(git branch --show-current)" != "main" ]; then
  echo "REFUSE: web OTA may be dispatched only from main" >&2
  exit 1
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "REFUSE: working tree must be clean" >&2
  exit 1
fi
if [ -n "$(git status --porcelain --untracked-files=normal)" ]; then
  echo "REFUSE: untracked files are present" >&2
  exit 1
fi

git fetch --quiet origin main
SOURCE_SHA="$(git rev-parse HEAD)"
REMOTE_MAIN="$(git rev-parse origin/main)"
if [ "$SOURCE_SHA" != "$REMOTE_MAIN" ]; then
  echo "REFUSE: HEAD must equal origin/main" >&2
  exit 1
fi

BASE_TAG="$(gh release view --json tagName -q .tagName)"
"$PYTHON" - "$BASE_TAG" "$WEB_VERSION" <<'PY'
import re, sys
tag, web = sys.argv[1:]
if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", tag):
    raise SystemExit("REFUSE: latest release tag is not vA.B.C")
base = tag[1:]
if not re.fullmatch(re.escape(base) + r"\.([1-9][0-9]*)", web):
    raise SystemExit("REFUSE: web version must be A.B.C.N for latest native tag, N > 0")
PY

SHORT_SHA="${SOURCE_SHA:0:12}"
BUNDLE="web-${WEB_VERSION}-${SHORT_SHA}.tar.gz"
MANIFEST="web-manifest-${WEB_VERSION}.json"
if gh release view "$BASE_TAG" --json assets --jq '.assets[].name' \
    | grep -Fqx -e "$BUNDLE" -e "$MANIFEST" -e "${MANIFEST}.sig"; then
  echo "REFUSE: one or more immutable assets for $WEB_VERSION already exist" >&2
  exit 1
fi

RUN_TITLE="Publish web OTA $WEB_VERSION from $SOURCE_SHA"
BEFORE_MAX="$(gh run list --workflow publish-web-ota.yml --limit 100 \
  --json databaseId --jq 'map(.databaseId) | max // 0')"

gh workflow run publish-web-ota.yml \
  --ref main \
  -f source_sha="$SOURCE_SHA" \
  -f base_tag="$BASE_TAG" \
  -f web_version="$WEB_VERSION"

RUN_ID=""
for _ in $(seq 1 90); do
  RUN_ID="$(gh run list --workflow publish-web-ota.yml --event workflow_dispatch \
    --branch main --limit 30 \
    --json databaseId,displayTitle,headSha \
    | "$PYTHON" -c '
import json, sys
title, sha, floor = sys.argv[1], sys.argv[2], int(sys.argv[3])
matches = [r for r in json.load(sys.stdin)
           if r["databaseId"] > floor
           and r["displayTitle"] == title
           and r["headSha"] == sha]
print(max((r["databaseId"] for r in matches), default=""))
' "$RUN_TITLE" "$SOURCE_SHA" "$BEFORE_MAX")"
  [ -n "$RUN_ID" ] && break
  sleep 2
done
if [ -z "$RUN_ID" ]; then
  echo "ERROR: dispatched workflow run was not observed" >&2
  exit 1
fi

echo "Waiting for signed web OTA workflow run $RUN_ID..."
gh run watch "$RUN_ID" --exit-status

VERIFY="$(mktemp -d)"
trap 'rm -rf "$VERIFY"' EXIT
gh release download "$BASE_TAG" \
  --pattern "$BUNDLE" \
  --pattern "$MANIFEST" \
  --pattern "${MANIFEST}.sig" \
  --dir "$VERIFY"
"$PYTHON" - "$VERIFY/$MANIFEST" "$VERIFY/$BUNDLE" "$VERIFY/${MANIFEST}.sig" \
  "$BASE_TAG" "$WEB_VERSION" "$SOURCE_SHA" "$MANIFEST" "$BUNDLE" <<'PY'
import base64, hashlib, json, pathlib, sys
manifest_path, bundle_path, signature_path = map(pathlib.Path, sys.argv[1:4])
tag, web_version, source_sha, manifest_name, bundle_name = sys.argv[4:]
doc = json.loads(manifest_path.read_text(encoding="utf-8"))
expected = {
    "schema": "hanni.web-ota.v1",
    "repository": "sultanjakhan/hanni",
    "channel": "stable",
    "release_tag": tag,
    "min_native_version": tag[1:],
    "web_version": web_version,
    "sequence": int(web_version.rsplit(".", 1)[1]),
    "source_commit": source_sha,
    "manifest_asset": manifest_name,
    "asset": bundle_name,
}
for key, value in expected.items():
    if doc.get(key) != value:
        raise SystemExit(f"remote manifest mismatch: {key}")
bundle = bundle_path.read_bytes()
if doc.get("asset_size") != len(bundle):
    raise SystemExit("remote bundle size mismatch")
if doc.get("asset_sha256") != hashlib.sha256(bundle).hexdigest():
    raise SystemExit("remote bundle hash mismatch")
encoded = signature_path.read_text(encoding="utf-8").strip()
decoded = base64.b64decode(encoded, validate=True).decode("utf-8")
lines = decoded.splitlines()
if len(lines) != 4 or not lines[2].startswith("trusted comment: timestamp:") \
        or not lines[2].endswith("\tfile:" + manifest_name):
    raise SystemExit("remote signature envelope mismatch")
PY
bash "$ROOT/scripts/verify-tauri-signature.sh" \
  "$VERIFY/$MANIFEST" "$VERIFY/${MANIFEST}.sig" \
  "$ROOT/desktop/src-tauri/updater.pub"

rm -rf "$VERIFY"
trap - EXIT
echo "Verified signed web OTA $WEB_VERSION in $BASE_TAG (run $RUN_ID)."
