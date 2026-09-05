#!/usr/bin/env python3
"""Compare pinned HC registry declarations with a real merged APK manifest.

Run against the final APK before preparing a Health Connect release.
Uses Python's standard library and the Android SDK's official aapt2 executable.
Actual HealthPermission.getReadPermission class mapping is independently tested
by RawHealthRecordCodecTest; this check closes the APK packaging boundary.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apk', type=Path, required=True)
    parser.add_argument('--aapt2', type=Path, required=True)
    parser.add_argument('--registry', type=Path,
        default=Path(__file__).resolve().parent.parent / 'desktop/src-tauri/android-plugin/src/main/java/com/sultanjakhan/hanni/RawHealthRecordCodec.kt')
    parser.add_argument('--expected-record-types', type=int, default=41)
    args = parser.parse_args()
    if not all(path.is_file() for path in [args.apk, args.aapt2, args.registry]):
        parser.error('APK, aapt2 and registry must be existing files')
    registry = re.findall(
        r'Descriptor\("([^\"]+)",\s*[^,]+,\s*"(android\.permission\.health\.[^\"]+)"',
        args.registry.read_text(encoding='utf-8'))
    names = {name for name, _ in registry}
    if len(registry) != args.expected_record_types or len(names) != len(registry):
        print(json.dumps({'ok': False, 'error': 'unexpected_or_duplicate_record_registry', 'record_types': len(registry)}))
        return 1
    result = subprocess.run([str(args.aapt2.resolve()), 'dump', 'badging', str(args.apk.resolve())],
        text=True, encoding='utf-8', capture_output=True)
    if result.returncode:
        print(json.dumps({'ok': False, 'error': 'aapt2_failed', 'exit_code': result.returncode}))
        return 1
    declared = set(re.findall(r"uses-permission(?:-sdk-\d+)?: name='(android\.permission\.health\.[^']+)'", result.stdout))
    required = {permission for _, permission in registry}
    history = 'android.permission.health.READ_HEALTH_DATA_HISTORY' in declared
    background = 'android.permission.health.READ_HEALTH_DATA_IN_BACKGROUND' in declared
    missing = sorted(required - declared)
    ok = not missing and history and background
    print(json.dumps({'ok': ok, 'record_types': len(registry),
        'distinct_record_read_permissions': len(required), 'merged_health_permission_count': len(declared),
        'missing_read_permissions': missing, 'history_permission': history, 'background_permission': background}))
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main())
