#!/usr/bin/env bash
# Hanni share-link → Firebase one-shot setup.
#
# Usage:
#   1. firebase login                # interactive — once
#   2. ./scripts/firebase-setup.sh   # creates project, enables Firestore + Hosting,
#                                    # deploys rules, prints config snippet
#
# Idempotent: safe to re-run.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIREBASE_DIR="$REPO_ROOT/firebase"
GUEST_DIR="$REPO_ROOT/apps/guest-cloud"

cd "$FIREBASE_DIR"

# --- Sanity checks ---------------------------------------------------------

if ! command -v firebase >/dev/null 2>&1; then
  echo "✘ firebase CLI not found. Install with: npm install -g firebase-tools" >&2
  exit 1
fi

if ! firebase login:list 2>/dev/null | grep -q '@'; then
  echo "✘ Not logged in. Run: firebase login" >&2
  exit 1
fi

# --- Project: create or reuse ---------------------------------------------

PROJECT_ID="${HANNI_FIREBASE_PROJECT:-}"
if [ -z "$PROJECT_ID" ]; then
  # Default: hanni-share-<8-char-hash> derived from machine id (idempotent per host)
  HOST_HASH=$(hostname | md5sum 2>/dev/null | cut -c1-8 || hostname | md5 | cut -c1-8)
  PROJECT_ID="hanni-share-$HOST_HASH"
fi

echo "▸ Project ID: $PROJECT_ID"

if firebase projects:list 2>/dev/null | grep -q "$PROJECT_ID"; then
  echo "  → exists, reusing"
else
  echo "  → creating new project (Spark plan, free tier)"
  firebase projects:create "$PROJECT_ID" --display-name "Hanni Share" \
    || { echo "✘ Project creation failed (Project ID may be taken). Set HANNI_FIREBASE_PROJECT and retry." >&2; exit 1; }
fi

# Update .firebaserc
cat > .firebaserc <<EOF
{ "projects": { "default": "$PROJECT_ID" } }
EOF

# --- Enable Firestore -----------------------------------------------------

echo "▸ Enabling Firestore (us-central1, native mode)"
firebase firestore:databases:list --project "$PROJECT_ID" 2>/dev/null \
  | grep -q '(default)' \
  || firebase firestore:databases:create '(default)' \
       --location us-central1 --type firestore-native --project "$PROJECT_ID"

# --- Deploy security rules + indexes --------------------------------------

echo "▸ Deploying Firestore rules + indexes"
firebase deploy --only firestore:rules,firestore:indexes --project "$PROJECT_ID"

# --- Set up Hosting (guest-cloud static site) -----------------------------

if [ -d "$GUEST_DIR" ] && [ -f "$GUEST_DIR/index.html" ]; then
  echo "▸ Deploying static guest UI to Firebase Hosting"
  firebase deploy --only hosting --project "$PROJECT_ID"
else
  echo "  → skipping Hosting deploy (apps/guest-cloud/index.html not present yet)"
fi

# --- Web SDK config -------------------------------------------------------

echo
echo "✓ Setup complete. Web SDK config:"
echo "  ────────────────────────────────────────────────"
firebase apps:sdkconfig WEB --project "$PROJECT_ID" 2>/dev/null \
  || (
       APP_ID=$(firebase apps:create WEB "Hanni Guest" --project "$PROJECT_ID" \
                | grep -oE 'App ID: [^ ]+' | awk '{print $3}')
       firebase apps:sdkconfig WEB "$APP_ID" --project "$PROJECT_ID"
     )
echo "  ────────────────────────────────────────────────"
echo
echo "▸ Hanni: open Settings → Облачные ссылки → вставь конфиг (когда фича включена)."
echo "▸ Guest URL: https://$PROJECT_ID.web.app/?t=<share_token>"
