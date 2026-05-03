#!/usr/bin/env python3
"""Deploy apps/guest-cloud/ to Firebase Hosting via REST API.

Avoids the firebase CLI (which needs interactive `firebase login`) by using
the project's service-account JSON to mint an OAuth access token with the
`firebase` scope, then talking to the Hosting REST API directly.

Pipeline (Firebase Hosting REST):
  1. POST   /v1beta1/sites/<site>/versions               → empty version
  2. walk apps/guest-cloud, gzip every file, sha256 hash
  3. POST   /v1beta1/sites/<site>/versions/<vid>:populateFiles
              { files: { "/path": "<sha256>", ... } }
              ← returns uploadUrl + uploadRequiredHashes (subset to upload)
  4. POST   <uploadUrl>/<hash>  with raw gzipped body, for each hash
  5. PATCH  /v1beta1/sites/<site>/versions/<vid>?updateMask=status
              { status: "FINALIZED" }
  6. POST   /v1beta1/sites/<site>/releases?versionName=sites/<site>/versions/<vid>

Usage:
  python3 scripts/firebase_hosting_deploy.py \\
      --sa ~/.config/hanni-firebase-sa-hanni-2e5d0.json \\
      --site hanni-2e5d0 \\
      --dir apps/guest-cloud
"""
import argparse
import gzip
import hashlib
import json
import os
import sys
import time
from pathlib import Path

import jwt
import requests

TOKEN_URL = "https://oauth2.googleapis.com/token"
HOSTING_API = "https://firebasehosting.googleapis.com/v1beta1"
SCOPE = "https://www.googleapis.com/auth/firebase"


def get_access_token(sa_path: Path) -> str:
    sa = json.loads(sa_path.read_text())
    now = int(time.time())
    claims = {
        "iss": sa["client_email"],
        "scope": SCOPE,
        "aud": sa.get("token_uri") or TOKEN_URL,
        "iat": now,
        "exp": now + 3500,
    }
    signed = jwt.encode(claims, sa["private_key"], algorithm="RS256")
    r = requests.post(claims["aud"], data={
        "grant_type": "urn:ietf:params:oauth:grant-type:jwt-bearer",
        "assertion": signed,
    }, timeout=30)
    r.raise_for_status()
    return r.json()["access_token"]


def collect_files(root: Path):
    """Yield (web_path, gzipped_bytes, sha256_hex) for every file under root."""
    skip = {".DS_Store"}
    for p in root.rglob("*"):
        if not p.is_file() or p.name in skip:
            continue
        rel = "/" + str(p.relative_to(root))
        raw = p.read_bytes()
        gz = gzip.compress(raw, compresslevel=9, mtime=0)
        h = hashlib.sha256(gz).hexdigest()
        yield rel, gz, h


def deploy(site: str, sa: Path, src: Path):
    token = get_access_token(sa)
    headers = {"Authorization": f"Bearer {token}"}
    base = f"{HOSTING_API}/sites/{site}"

    # 1. Create version with rewrites + cache headers (SPA-style routing for
    #    /s/<token>; everything not a real file falls through to index.html).
    print(f"→ creating version on site '{site}'...")
    version_config = {
        "config": {
            "rewrites": [{"glob": "**", "path": "/index.html"}],
            "headers": [
                {"glob": "/index.html", "headers": {"Cache-Control": "no-cache"}},
                {"glob": "**", "headers": {"Cache-Control": "public, max-age=300"}},
            ],
        }
    }
    r = requests.post(f"{base}/versions", headers=headers, json=version_config, timeout=30)
    r.raise_for_status()
    version_name = r.json()["name"]   # "sites/hanni-2e5d0/versions/<vid>"
    vid = version_name.split("/")[-1]
    print(f"  version: {version_name}")

    # 2-3. Hash + populateFiles
    files_by_hash = {}
    files_manifest = {}
    for path, gz, h in collect_files(src):
        files_manifest[path] = h
        files_by_hash[h] = (path, gz)
    print(f"→ populating manifest of {len(files_manifest)} files...")
    r = requests.post(
        f"{HOSTING_API}/{version_name}:populateFiles",
        headers=headers,
        json={"files": files_manifest},
        timeout=60,
    )
    r.raise_for_status()
    body = r.json()
    upload_url = body["uploadUrl"]
    required = body.get("uploadRequiredHashes", []) or []
    print(f"  uploadUrl: {upload_url}")
    print(f"  required uploads: {len(required)} (server already has {len(files_manifest) - len(required)})")

    # 4. Upload each required hash
    for i, h in enumerate(required, 1):
        path, gz = files_by_hash[h]
        ur = requests.post(
            f"{upload_url}/{h}",
            headers={**headers, "Content-Type": "application/octet-stream"},
            data=gz,
            timeout=120,
        )
        if not ur.ok:
            print(f"  ✗ upload {h[:12]} ({path}): {ur.status_code} {ur.text[:200]}")
            ur.raise_for_status()
        print(f"  [{i}/{len(required)}] {path}  ({h[:12]}…)")

    # 5. Finalize
    print("→ finalizing version...")
    r = requests.patch(
        f"{HOSTING_API}/{version_name}?updateMask=status",
        headers=headers,
        json={"status": "FINALIZED"},
        timeout=30,
    )
    r.raise_for_status()

    # 6. Release
    print("→ releasing version...")
    r = requests.post(
        f"{base}/releases",
        headers=headers,
        params={"versionName": version_name},
        timeout=30,
    )
    r.raise_for_status()
    rel = r.json()
    print(f"  release: {rel.get('name')}")
    print()
    print(f"✓ Deployed.  Live at:  https://{site}.web.app/")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sa", required=True, help="service account JSON path")
    ap.add_argument("--site", required=True, help="hosting site id (project_id by default)")
    ap.add_argument("--dir", required=True, help="folder to upload (apps/guest-cloud)")
    args = ap.parse_args()
    deploy(args.site, Path(os.path.expanduser(args.sa)), Path(args.dir))


if __name__ == "__main__":
    main()
