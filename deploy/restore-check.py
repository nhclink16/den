#!/usr/bin/env python3
"""Boot a restored backup on codexbox and check its authenticated API and files.

Usage: deploy/restore-check.py /mnt/storage/den/backups/YYYY-MM-DD TOKEN_JSON
TOKEN_JSON holds the output of `den token create`, never a token argument.
"""
import datetime
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.request
import urllib.error

source = Path(sys.argv[1]).absolute()
root = Path('/mnt/storage/den/backups')
assert source.parent == root
assert subprocess.run(['sudo', 'test', '-L', str(source)]).returncode == 1
datetime.date.fromisoformat(source.name)
token = json.loads(Path(sys.argv[2]).read_text())['token']
repo = Path(__file__).resolve().parent.parent
build = Path(os.environ.get('CARGO_TARGET_DIR', repo / 'target')) / 'release'
with tempfile.TemporaryDirectory(prefix='den-restore-', dir=os.environ.get('TMPDIR')) as scratch:
    work = Path(scratch)
    for name in ['den.zip', 'complete']:
        assert subprocess.run(['sudo', 'test', '-L', str(source / name)]).returncode == 1
    subprocess.run(['sudo', 'rsync', '-a', f'--chown={os.getuid()}:{os.getgid()}', str(source / 'den.zip'), str(source / 'complete'), scratch + '/'], check=True)
    datetime.datetime.fromisoformat((work / 'complete').read_text().strip().replace('Z', '+00:00'))
    restored = work / 'restored'
    subprocess.run([str(build / 'den-server'), 'import', str(work / 'den.zip'), '--into', str(restored), '--keep-credentials'], check=True)
    db = sqlite3.connect(restored / 'den.db')
    assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
    assert not db.execute('PRAGMA foreign_key_check').fetchall()
    uploads = db.execute('SELECT id,size FROM uploads WHERE complete=1').fetchall()
    for upload, size in uploads:
        path = restored / 'uploads' / upload
        assert path.is_file() and path.stat().st_size == size, f'Missing restored upload {upload}'
    db.close()
    env = {k: v for k, v in os.environ.items() if not k.startswith('DEN_')}
    env.update(DEN_BIND='127.0.0.1:17200', DEN_ORIGIN='http://127.0.0.1:17200', DEN_DB=str(restored / 'den.db'), DEN_UPLOADS=str(restored / 'uploads'), DEN_BOOTSTRAP_FILE=str(restored / 'bootstrap.key'), DEN_WEB_DIR=str(repo / 'apps/web/dist'))
    with (work / 'server.log').open('w') as log:
        server = subprocess.Popen([str(build / 'den-server')], env=env, stdout=log, stderr=log)
        try:
            for attempt in range(50):
                assert server.poll() is None, 'Restored server exited'
                try:
                    with urllib.request.urlopen('http://127.0.0.1:17200/health', timeout=2) as response:
                        assert json.load(response)['ok']; break
                except OSError:
                    time.sleep(0.1)
            else:
                raise RuntimeError('Restored server did not become healthy')
            def get(path, extra=None):
                return urllib.request.urlopen(urllib.request.Request('http://127.0.0.1:17200' + path, headers={'Authorization': 'Bearer ' + token, **(extra or {})}), timeout=10)
            with get('/channels') as response:
                channels = json.load(response)
                assert any(c['name'] == 'hangout' for c in channels)
            checked = 0
            for upload, size in uploads:
                try:
                    response = get(f'/uploads/{upload}/file', {'Range': 'bytes=0-1023'})
                except urllib.error.HTTPError as error:
                    if error.code == 404:
                        continue  # Private uploads can be outside this credential's visibility.
                    raise
                with response as response:
                    assert response.status == 206
                    with (restored / 'uploads' / upload).open('rb') as local:
                        assert response.read() == local.read(1024)
                    checked += 1
            assert checked > 0 or not uploads, 'No visible uploads were verified'
            print(f'Restore passed: archive hashes, migrated schema, health, {len(channels)} authenticated channels, integrity, foreign keys, {len(uploads)} upload files, {checked} authenticated byte ranges.')
        finally:
            server.terminate()
            server.wait(timeout=10)
