#!/usr/bin/env bash
# Remove only GitHub's documented empty `starter` placeholders so an exact
# immutable release asset upload can be retried after an upstream 502.
set -euo pipefail

if [ "$#" -lt 2 ]; then
  echo "usage: recover-starter-release-assets.sh <tag> <expected-name> [...]" >&2
  exit 2
fi
if [ -z "${GITHUB_REPOSITORY:-}" ]; then
  echo "GITHUB_REPOSITORY is required" >&2
  exit 1
fi

TAG="$1"
shift

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
  echo "a working Python 3.9+ interpreter is required for release recovery" >&2
  exit 1
fi

RELEASE_ID=$(gh api "repos/$GITHUB_REPOSITORY/releases/tags/$TAG" --jq .id)
ASSETS_JSON=$(mktemp)
trap 'rm -f "$ASSETS_JSON"' EXIT
gh api --paginate --slurp \
  "repos/$GITHUB_REPOSITORY/releases/$RELEASE_ID/assets?per_page=100" \
  > "$ASSETS_JSON"

STARTERS=$("$PYTHON" - "$ASSETS_JSON" "$@" <<'PY'
import json
import pathlib
import sys

pages = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
assets = [asset for page in pages for asset in page]
for expected in sys.argv[2:]:
    matches = [asset for asset in assets if asset.get("name") == expected]
    if len(matches) > 1:
        raise SystemExit(f"duplicate release assets named {expected}")
    if not matches:
        continue
    asset = matches[0]
    state = asset.get("state")
    size = asset.get("size")
    asset_id = asset.get("id")
    if state == "uploaded" and isinstance(size, int) and size > 0:
        continue
    if state == "starter" and size == 0 and isinstance(asset_id, int):
        print(f"{asset_id}\t{expected}")
        continue
    raise SystemExit(f"release asset {expected} has unsafe state/size")
PY
)

if [ -n "$STARTERS" ]; then
  while IFS=$'\t' read -r asset_id expected_name; do
    if ! [[ "$asset_id" =~ ^[0-9]+$ ]]; then
      echo "invalid starter release asset id" >&2
      exit 1
    fi
    CURRENT=$(gh api "repos/$GITHUB_REPOSITORY/releases/assets/$asset_id" \
      --jq '[.name, .state, .size] | @tsv')
    IFS=$'\t' read -r current_name current_state current_size <<< "$CURRENT"
    if [ "$current_name" != "$expected_name" ] \
       || [ "$current_state" != starter ] \
       || [ "$current_size" != 0 ]; then
      echo "starter release asset changed during recovery; refusing deletion" >&2
      exit 1
    fi
    gh api --method DELETE "repos/$GITHUB_REPOSITORY/releases/assets/$asset_id"
  done <<< "$STARTERS"
fi
