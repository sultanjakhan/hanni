#!/usr/bin/env bash
# publish-web-update.sh — push a JS/CSS/HTML-only OTA web-asset update WITHOUT
# rebuilding/redistributing the ~106MB APK / ~128MB .app. On next launch the app
# (Android + macOS) pulls this few-MB bundle (web_assets.rs hanniweb:// protocol)
# and serves it over the embedded assets. Big rarely-changing files (sounds/,
# assets/) are excluded and fall back to the embedded copies per-file.
#
# The bundle attaches to the LATEST GitHub release — the one clients read via
# releases/latest — WITHOUT a new tag or native-version bump, so the native
# updater stays silent and only the web OTA channel fires.
#
# Use for frontend-only changes. If you changed Rust/Kotlin/native assets, cut a
# full release instead: scripts/release.sh native <X.Y.Z>. Normally you don't
# call this directly — scripts/release.sh web does (after classifying the diff).
#
# Usage: scripts/publish-web-update.sh [web_version]
#   web_version  optional; auto-derived from the latest release's web-manifest.json
#                (A.B.C -> A.B.C.1 ; A.B.C.N -> A.B.C.(N+1)). Pass to override.
# Env:
#   MIN_NATIVE   override min_native_version (default: the latest release version,
#                i.e. the native shell this bundle is built against — safe floor).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Base = the release clients actually read (releases/latest). NOT `git describe`,
# which can point at a tag that never produced a release (CI failure / drift).
BASE_TAG="$(gh release view --json tagName -q .tagName)"
BASE_VER="${BASE_TAG#v}"

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT

# Pull the current web-manifest from that release to derive the next web_version.
CUR_WEB=""
if gh release download "$BASE_TAG" --pattern web-manifest.json --dir "$WORK" --clobber 2>/dev/null; then
  CUR_WEB="$(python3 -c "import json;print(json.load(open('$WORK/web-manifest.json')).get('web_version',''))" 2>/dev/null || true)"
fi

derive_next() {
  python3 - "$BASE_VER" "${CUR_WEB}" <<'PY'
import sys
base, cur = sys.argv[1], sys.argv[2]
def parts(s):
    return [int(x) if x.isdigit() else 0 for x in s.split('.')] if s else []
def gte(x, y):
    xs, ys = parts(x), parts(y)
    for i in range(max(len(xs), len(ys))):
        a = xs[i] if i < len(xs) else 0
        d = ys[i] if i < len(ys) else 0
        if a != d:
            return a > d
    return True
b = parts(base)
if not cur:
    nxt = base + ".1"
else:
    c = parts(cur)
    if c[:len(b)] != b:                       # manifest base must match the release
        sys.exit("REFUSE: manifest web_version %r base != release %r" % (cur, base))
    if len(c) == len(b):                      # cur == base (A.B.C) -> A.B.C.1
        nxt = base + ".1"
    else:                                     # cur == base.N... -> bump last part
        nxt = ".".join(map(str, c[:-1] + [c[-1] + 1]))
ref = cur if cur else base
if not (gte(nxt, ref) and nxt != ref):
    sys.exit("REFUSE: derived %r not strictly newer than %r" % (nxt, ref))
print(nxt)
PY
}

WEB_VERSION="${1:-$(derive_next)}"
MIN_NATIVE="${MIN_NATIVE:-$BASE_VER}"   # bundle built against BASE native -> safe floor

BUNDLE="web-${WEB_VERSION}.tar.gz"
OUT="$WORK/$BUNDLE"

# Build from the committed tree (HEAD), NOT the working tree, so what we ship
# equals what release.sh classified. Flat tarball (index.html at root); exclude
# the heavy static dirs (they fall back to the embedded copies per-file).
STAGE="$WORK/src"; mkdir -p "$STAGE"
git archive HEAD:desktop/src | tar -x -C "$STAGE"
tar czf "$OUT" -C "$STAGE" --exclude=./sounds --exclude=./assets .
SHA="$(shasum -a 256 "$OUT" | cut -d' ' -f1)"
printf '{"web_version":"%s","min_native_version":"%s","sha256":"%s","asset":"%s"}\n' \
  "$WEB_VERSION" "$MIN_NATIVE" "$SHA" "$BUNDLE" > "$WORK/web-manifest.json"

echo "base release : $BASE_TAG (native $BASE_VER)"
echo "current web  : ${CUR_WEB:-<none>}"
echo "new web_ver  : $WEB_VERSION (min_native $MIN_NATIVE)"
echo "bundle       : $BUNDLE ($(du -h "$OUT" | cut -f1)) sha256=$SHA"
echo "manifest     : $(cat "$WORK/web-manifest.json")"

# DRY=1 → preview only: show what would ship, upload nothing.
if [ -n "${DRY:-}" ]; then
  echo "(DRY) would: gh release upload $BASE_TAG $BUNDLE web-manifest.json --clobber — skipped"
  exit 0
fi

gh release upload "$BASE_TAG" "$OUT" "$WORK/web-manifest.json" --clobber

# Read the manifest back to catch a clobber race (e.g. a concurrent CI run that
# re-published the bare-version bundle over ours).
gh release download "$BASE_TAG" --pattern web-manifest.json --dir "$WORK/verify" --clobber
GOT="$(python3 -c "import json;print(json.load(open('$WORK/verify/web-manifest.json'))['web_version'])")"
if [ "$GOT" != "$WEB_VERSION" ]; then
  echo "ERROR: manifest on release is $GOT, expected $WEB_VERSION (clobber race?)" >&2
  exit 1
fi
echo "✓ published web update $WEB_VERSION to release $BASE_TAG (verified)"
