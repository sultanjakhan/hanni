#!/usr/bin/env python3
"""Check the signed candidate's source binding, file set, hashes, and PE images."""
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
stage = Path(sys.argv[1])
report = json.loads((stage / 'verification.json').read_text(encoding='utf-8-sig'))
assert report['schema'] == 'hanni.windows-health-candidate.v1'
assert report['source_commit'] == os.environ['GITHUB_SHA']
assert report['source_tree'] == subprocess.check_output(['git', 'rev-parse', 'HEAD^{tree}'], cwd=root, text=True).strip()
assert report['version'] == report['pe_file_version'] == report['pe_product_version'] == '1.1.15'
assert report['build_profile'] == 'release'
assert report['target'] == 'x86_64-pc-windows-msvc'
assert report['installed'] is False
for key, path in [('config_sha256', 'desktop/src-tauri/tauri.health-candidate.conf.json'), ('updater_pub_sha256', 'desktop/src-tauri/updater.pub')]:
    assert report[key] == hashlib.sha256((root / path).read_bytes()).hexdigest()
names = []
for row in report['files']:
    name = row['name']
    assert re.fullmatch(r'[A-Za-z0-9_.-]+\.(?:exe|dll)', name, re.I)
    assert name.lower() not in [value.lower() for value in names]
    names.append(name)
    payload = (stage / name).read_bytes()
    assert len(payload) == row['bytes']
    assert hashlib.sha256(payload).hexdigest() == row['sha256']
    assert payload[:2] == b'MZ'
    pe = struct.unpack_from('<I', payload, 0x3c)[0]
    assert payload[pe:pe+4] == b'PE\0\0'
    assert struct.unpack_from('<H', payload, pe+4)[0] == 0x8664
assert 'hanni.exe' in names
expected = {'verification.json', 'verification.json.sig'}
expected.update(names)
expected.update(name + '.sig' for name in names)
assert {path.name for path in stage.iterdir()} == expected
assert all(path.is_file() and path.stat().st_size for path in stage.iterdir())
print(json.dumps({'source_commit': report['source_commit'], 'source_tree': report['source_tree'], 'version': report['version'], 'signed_payload_count': len(names), 'hashes_and_pe': 'PASS', 'installed': False}))
