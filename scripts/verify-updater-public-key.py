#!/usr/bin/env python3
"""Fail if the native client key and committed Minisign key diverge."""

from __future__ import annotations

import base64
import binascii
import json
import pathlib


ROOT = pathlib.Path(__file__).resolve().parents[1]
CONFIG = ROOT / "desktop" / "src-tauri" / "tauri.conf.json"
PUBLIC_KEY = ROOT / "desktop" / "src-tauri" / "updater.pub"


def main() -> None:
    config = json.loads(CONFIG.read_text(encoding="utf-8"))
    encoded = config.get("plugins", {}).get("updater", {}).get("pubkey")
    if not isinstance(encoded, str) or not encoded:
        raise SystemExit("tauri.conf.json does not contain an updater public key")
    try:
        embedded = base64.b64decode("".join(encoded.split()), validate=True)
    except (binascii.Error, ValueError) as error:
        raise SystemExit("tauri.conf.json updater public key is not valid base64") from error
    committed = PUBLIC_KEY.read_bytes()
    if embedded != committed:
        raise SystemExit("embedded updater key differs from desktop/src-tauri/updater.pub")


if __name__ == "__main__":
    main()
