#!/usr/bin/env python3
"""Run on codexbox. The VPS's restricted key cannot delete old backups."""
import datetime
import pathlib
import shutil

root = pathlib.Path('/mnt/storage/den/backups')
cutoff = datetime.datetime.now(datetime.timezone.utc).date() - datetime.timedelta(days=13)
for path in root.iterdir():
    if path.is_symlink() or not path.is_dir():
        continue
    try:
        day = datetime.date.fromisoformat(path.name)
    except ValueError:
        continue
    if day < cutoff:
        shutil.rmtree(path)
