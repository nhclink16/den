#!/usr/bin/env python3
"""Run as root on the VPS. Replaces API credentials and disconnects active calls."""
import os
from pathlib import Path
import secrets
import subprocess

os.umask(0o077)
env = Path('/etc/den/den.env')
yaml = Path('/etc/livekit/livekit.yaml')
values = dict(line.split('=', 1) for line in env.read_text().splitlines() if '=' in line)
key, secret = 'API' + secrets.token_hex(12), secrets.token_urlsafe(48)
for path in [env, yaml]:
    stat = path.stat()
    content = path.read_text().replace(values['DEN_LIVEKIT_API_KEY'], key).replace(values['DEN_LIVEKIT_API_SECRET'], secret)
    temporary = path.with_suffix('.new')
    temporary.write_text(content)
    temporary.chmod(0o600)
    os.chown(temporary, stat.st_uid, stat.st_gid)
    temporary.replace(path)
subprocess.run(['systemctl', 'restart', 'livekit', 'den-server'], check=True)
print('LiveKit API credentials rotated; Den and LiveKit restarted.')
