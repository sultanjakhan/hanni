#!/usr/bin/env bash
# publish-web-update.sh — push a JS/CSS/HTML-only OTA web-asset update WITHOUT
# rebuilding/redistributing the ~106MB APK. On next launch the Android app pulls
# this few-MB bundle (web_assets.rs custom protocol) and serves it over the
# embedded assets. Big rarely-changing files (sounds/, assets/) are excluded and
# fall back to the APK-embedded copies per-file.
#
# Use this for frontend-only changes. If you changed Rust/Kotlin, cut a normal
# release instead (the APK + a web bundle both ship from release-android.yml).
#
# Usage: scripts/publish-web-update.sh <web_version> [release_tag]
#   <web_version>  e.g. 0.91.2.1 — MUST sort newer than the applied web_version
#                  (numeric dot-compare; bundle is applied only if newer +
#                  min_native_version <= installed APK version).
#   [release_tag]  GitHub release to attach assets to (default: latest).
set -euo pipefail

WEB_VERSION="${1:?usage: publish-web-update.sh <web_version> [release_tag]}"
TAG="${2:-$(gh release view --json tagName -q .tagName)}"
# Bump if a web bundle ever needs a Rust command newer than this native version.
MIN_NATIVE="0.91.2"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE="web-${WEB_VERSION}.tar.gz"
OUT="/tmp/$BUNDLE"

# Flat tarball (index.html at root). Exclude the heavy static assets.
tar czf "$OUT" -C "$ROOT/desktop/src" --exclude=./sounds --exclude=./assets .
SHA=$(shasum -a 256 "$OUT" | cut -d' ' -f1)
printf '{"web_version":"%s","min_native_version":"%s","sha256":"%s","asset":"%s"}\n' \
  "$WEB_VERSION" "$MIN_NATIVE" "$SHA" "$BUNDLE" > /tmp/web-manifest.json

echo "bundle: $OUT ($(du -h "$OUT" | cut -f1)) sha256=$SHA"
echo "manifest: $(cat /tmp/web-manifest.json)"
gh release upload "$TAG" "$OUT" /tmp/web-manifest.json --clobber
echo "✓ published web update $WEB_VERSION to release $TAG"
