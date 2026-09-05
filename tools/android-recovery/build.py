"""SDK-only recovery build. Output must be outside source; signing only in existing CI."""
import argparse, base64, hashlib, json, os, re, subprocess, sys, tempfile, zipfile
from pathlib import Path
import xml.etree.ElementTree as ET

CERT = 'F0DD5C4C931FA2A8EBE2D223136D71293DBDE7EA59130615B6B5EB11FC530C63'
ROOT = Path(__file__).resolve().parent
def run(args, cwd=None): return subprocess.check_output([str(v) for v in args], stderr=subprocess.PIPE, cwd=cwd)
def sha(data): return hashlib.sha256(data).hexdigest()
def build(args):
    out = args.output.resolve(); out.mkdir(parents=True, exist_ok=False)
    assert not out.is_relative_to(ROOT)
    sdk = args.sdk.resolve(); tools = sdk/'build-tools'/'36.0.0'; jar = sdk/'platforms'/'android-36'/'android.jar'
    java = args.java.resolve(); suffix = '.exe' if os.name == 'nt' else ''
    public = base64.b64decode(os.environ['RECIPIENT_PUBLIC_KEY'], validate=True); assert 0 < len(public) <= 8192
    assets = out/'assets'; assets.mkdir(); (assets/'recipient.der').write_bytes(public)
    description = run(['openssl','pkey','-pubin','-inform','DER','-in',assets/'recipient.der','-text','-noout'])
    assert re.search(rb'Public-Key: \((3072|4096) bit\)', description)
    canonical = run(['openssl','pkey','-pubin','-inform','DER','-in',assets/'recipient.der','-pubout','-outform','DER'])
    assert canonical == public
    classes = out/'classes'; classes.mkdir(); dex = out/'dex'; dex.mkdir()
    sources = sorted((ROOT/'src').rglob('*.java')); assert len(sources) == 2
    run([java/('javac'+suffix),'-encoding','UTF-8','-source','8','-target','8','-cp',jar,'-d',classes,*sources])
    run([java/('java'+suffix),'-cp',tools/'lib'/'d8.jar','com.android.tools.r8.D8','--release','--min-api','26','--lib',jar,'--output',dex,*sorted(classes.rglob('*.class'))])
    unsigned = out/'unsigned.apk'
    run([tools/('aapt2'+suffix),'link','--manifest',os.path.relpath(ROOT/'AndroidManifest.xml',out),'-I',jar,'-A','assets','-o',unsigned.name], cwd=out)
    with zipfile.ZipFile(unsigned, 'a', compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(dex.glob('*.dex')): archive.write(path, path.name)
    aligned = out/'aligned.apk'; run([tools/('zipalign'+suffix),'-f','4',unsigned.name,aligned.name], cwd=out)
    apk = out/'hanni-preservation-1.1.15.apk'
    if args.sign:
        assert os.environ.get('GITHUB_ACTIONS') == 'true'
        with tempfile.TemporaryDirectory(prefix='hanni-signing-',dir=os.environ['RUNNER_TEMP']) as private_dir:
            private = Path(private_dir); private.chmod(0o700)
            keystore = private/'release.jks'; keystore.write_bytes(base64.b64decode(os.environ['ANDROID_KEYSTORE_BASE64'], validate=True)); keystore.chmod(0o600)
            run([java/('java'+suffix),'-jar',tools/'lib'/'apksigner.jar','sign','--ks',keystore,'--ks-key-alias',os.environ['ANDROID_KEY_ALIAS'],'--ks-pass','env:ANDROID_KEYSTORE_PASSWORD','--key-pass','env:ANDROID_KEY_PASSWORD','--out',apk,aligned])
        verification = run([java/('java'+suffix),'-jar',tools/'lib'/'apksigner.jar','verify','--verbose','--print-certs',apk])
        certs = re.findall(rb'Signer #[0-9]+ certificate SHA-256 digest: ([0-9a-fA-F]+)',verification)
        assert len(certs) == 1 and certs[0].decode().upper() == CERT
    else: apk.write_bytes(aligned.read_bytes())
    manifest = run([tools/('aapt2'+suffix),'dump','xmltree',apk.name,'--file','AndroidManifest.xml'], cwd=out).decode()
    assert len(re.findall(r'E: activity \(',manifest)) == 1
    assert not re.search(r'E: (provider|receiver|service|activity-alias|instrumentation) \(',manifest)
    assert 'com.sultanjakhan.hanni.recovery.RecoveryActivity' in manifest and 'android.app.Application' in manifest
    assert re.search(r':debuggable\([^)]*\)=false',manifest) and re.search(r':allowBackup\([^)]*\)=false',manifest)
    assert 'android.permission.DUMP' in manifest
    badging = run([tools/('aapt2'+suffix),'dump','badging',apk.name], cwd=out).decode()
    assert "name='com.sultanjakhan.hanni'" in badging and "versionCode='1001015'" in badging and "versionName='1.1.15'" in badging
    expected = {node.attrib['{http://schemas.android.com/apk/res/android}name'] for node in ET.parse(ROOT/'AndroidManifest.xml').getroot().findall('uses-permission')}
    actual = set(re.findall(r"uses-permission: name='([^']+)'",badging)); assert expected == actual
    with zipfile.ZipFile(apk) as archive:
        assert not any(name.startswith('lib/') for name in archive.namelist())
        assert archive.read('assets/recipient.der') == public
        data = b''.join(archive.read(name) for name in archive.namelist() if name.endswith('.dex'))
        assert all(token not in data for token in [b'SQLiteDatabase',b'androidx/work',b'androidx/startup',b'hanni_lib',b'Lcom/sultanjakhan/hanni/MainActivity;'])
    result = dict(schema='hanni.forensic-apk.v1', apk=apk.name, apk_sha256=sha(apk.read_bytes()), apk_bytes=apk.stat().st_size,
        version='1.1.15', version_code=1001015, package='com.sultanjakhan.hanni', production_signature_verified=args.sign,
        expected_signer_sha256=CERT, recipient_public_key_sha256=sha(public), sole_activity='com.sultanjakhan.hanni.recovery.RecoveryActivity',
        collector_port_intent='collector_port', public_asset='recipient.der', automatic_components=0, native_libraries=0,
        health_permissions=sum(p.startswith('android.permission.health.') for p in expected), installed=False)
    if args.sign:
        result.update(source_commit=run(['git','rev-parse','HEAD']).decode().strip(), source_tree=run(['git','rev-parse','HEAD^{tree}']).decode().strip())
    (out/'verification.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))

if __name__ == '__main__':
    parser=argparse.ArgumentParser(); parser.add_argument('--sdk',type=Path,required=True); parser.add_argument('--java',type=Path,required=True); parser.add_argument('--output',type=Path,required=True); parser.add_argument('--sign',action='store_true')
    try: build(parser.parse_args())
    except Exception: print('{"ok":false,"error":"recovery_build_failed"}'); sys.exit(1)
