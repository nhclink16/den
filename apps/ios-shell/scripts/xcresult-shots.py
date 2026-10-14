"""Save the screenshots in an Xcode Cloud result bundle as PNG files, on Linux (no xcresulttool needed).
  python3 xcresult-shots.py <result-bundle.zip> <out-dir>
A bundle keeps each attachment as Data/data.<id>, zstd-compressed, and names them in database.sqlite3.
Needs the zstd command.
"""
import os, sqlite3, subprocess, sys, tempfile, zipfile

ZSTD = b'\x28\xb5\x2f\xfd'
PNG = b'\x89PNG'


def payload(data_dir, ref):
    raw = open(os.path.join(data_dir, f'data.{ref}'), 'rb').read()
    return subprocess.run(['zstd', '-dcq'], input=raw, capture_output=True, check=True).stdout if raw[:4] == ZSTD else raw


def save(bundle, out):
    data_dir = os.path.join(bundle, 'Data')
    db = sqlite3.connect(os.path.join(bundle, 'database.sqlite3'))
    rows = db.execute('''SELECT TestCases.name, Attachments.name, Attachments.xcResultKitPayloadRefId
        FROM Attachments
        JOIN Activities ON Activities.rowid = Attachments.activity_fk
        JOIN TestCaseRuns ON TestCaseRuns.rowid = Activities.testCaseRun_fk
        JOIN TestCases ON TestCases.rowid = TestCaseRuns.testCase_fk
        ORDER BY Attachments.rowid''').fetchall()
    for test, name, ref in rows:
        body = payload(data_dir, ref)
        test = test.removesuffix('()')
        if body[:4] == PNG:
            path = os.path.join(out, f'{test}-{name}.png')
        elif name.startswith('tree-'):
            path = os.path.join(out, f'{test}-{name}.txt')
        else:
            continue
        open(path, 'wb').write(body)
        print(path)


zip_path, out = sys.argv[1:3]
os.makedirs(out, exist_ok=True)
with tempfile.TemporaryDirectory() as tmp:
    zipfile.ZipFile(zip_path).extractall(tmp)
    for root, dirs, _ in os.walk(tmp):
        for d in dirs:
            if d.endswith('.xcresult'):
                save(os.path.join(root, d), out)
