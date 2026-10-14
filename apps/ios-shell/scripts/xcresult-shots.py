"""Save the attachments (screenshots, text) in an Xcode Cloud result bundle as files, on Linux: no xcresulttool.
  python3 xcresult-shots.py <result-bundle.zip> <out-dir>
Writes <out-dir>/<test>-<attachment name>.<png|txt>. Needs the zstd command.

A bundle keeps every object as Data/data.<id>, mostly zstd-compressed. Each test's summary is one of them, in a
text form where every value is length-prefixed ("_vV11:1-room-list"); its attachments name the data file
that holds their payload ("payloadRef ... id").
"""
import glob, os, re, subprocess, sys, tempfile, zipfile


def read(path):
    raw = open(path, 'rb').read()
    return subprocess.run(['zstd', '-dcq'], input=raw, capture_output=True, check=True).stdout \
        if raw[:4] == b'\x28\xb5\x2f\xfd' else raw


def value(text, key, start):
    """The length-prefixed value that follows `key` at or after `start`."""
    at = text.index(key, start)
    m = re.compile(rb'_vV(\d+):').search(text, at)
    return m.end() + int(m[1]), text[m.end():m.end() + int(m[1])].decode()


def save(bundle, out):
    for path in glob.glob(os.path.join(bundle, 'Data', 'data.*')):
        text = read(path)
        if not text.startswith(b'[T[K2:_nV17:ActionTestSummary'):
            continue
        _, test = value(text, b'K10:identifier', 0)
        test = test.split('/')[-1].removesuffix('()')
        at = 0
        while (at := text.find(b'K8:filename', at)) >= 0:
            at, filename = value(text, b'K8:filename', at)
            at, name = value(text, b'K4:name', at)
            at, ref = value(text, b'K2:id', text.index(b'K10:payloadRef', at))
            # Xcode's own (a screen recording, UI snapshots on failure) are large and repeat the test's own.
            if name.startswith(('kXCT', 'Debug description', 'App UI hierarchy')):
                continue
            ext = os.path.splitext(filename)[1] or '.bin'
            dest = os.path.join(out, f'{test}-{name}{ext}')
            open(dest, 'wb').write(read(os.path.join(bundle, 'Data', f'data.{ref}')))
            print(dest)


zip_path, out = sys.argv[1:3]
os.makedirs(out, exist_ok=True)
with tempfile.TemporaryDirectory() as tmp:
    zipfile.ZipFile(zip_path).extractall(tmp)
    for bundle in glob.glob(os.path.join(tmp, '**', '*.xcresult'), recursive=True):
        save(bundle, out)
