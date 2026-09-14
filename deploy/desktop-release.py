#!/usr/bin/env python3
"""Flatten CI bundles and make the updater manifest from the actual signed files."""
import datetime
import json
from pathlib import Path
import sys

root = Path('release-artifacts')
tag = sys.argv[1]
for p in list(root.rglob('*')):
    if p.is_file() and p.parent != root:
        destination = root / p.name
        if destination.exists():
            if destination.read_bytes() != p.read_bytes():
                raise SystemExit(f'Conflicting release artifact: {p.name}')
            p.unlink()
        else:
            p.rename(destination)
for p in sorted(root.rglob('*'), key=lambda p: len(p.parts), reverse=True):
    if p.is_dir():
        p.rmdir()
platforms = {}
for pattern, targets in [
    ('*.AppImage.sig', ['linux-x86_64']),
    ('*.app.tar.gz.sig', ['darwin-aarch64', 'darwin-x86_64']),
    ('*.exe.sig', ['windows-x86_64']),
]:
    matches = list(root.glob(pattern))
    if len(matches) != 1:
        raise SystemExit(f'Expected one {pattern}, found {len(matches)}')
    sig = matches[0]
    artifact = sig.name.removesuffix('.sig')
    for target in targets:
        platforms[target] = {'signature': sig.read_text().strip(), 'url': f'https://github.com/nhclink16/den/releases/download/{tag}/{artifact}'}
(root / 'latest.json').write_text(json.dumps({'version': tag.removeprefix('v'), 'notes': f'Den {tag}', 'pub_date': datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00', 'Z'), 'platforms': platforms}, indent=2) + '\n')
