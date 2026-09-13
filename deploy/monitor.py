#!/usr/bin/env python3
"""Public health probe using the existing Hermes Telegram delivery command."""
import json
import pathlib
import subprocess

URL = 'https://den.nicholascaron.com/health'
STATE = pathlib.Path.home() / '.local/state/den-monitor/state.json'


def advance(previous, healthy):
    failures = 0 if healthy else previous.get('failures', 0) + 1
    alerted = previous.get('alerted', False)
    message = None
    if failures >= 2 and not alerted:
        message = f'Den is down: {URL} failed two consecutive five-minute checks.'
        alerted = True
    elif healthy and alerted:
        message = f'Den recovered: {URL} is healthy again.'
        alerted = False
    return {'failures': failures, 'alerted': alerted}, message


def main():
    result = subprocess.run(['curl', '--fail', '--silent', '--show-error', '--max-time', '20', URL], capture_output=True, text=True)
    try:
        healthy = result.returncode == 0 and json.loads(result.stdout).get('ok') is True
    except (ValueError, AttributeError):
        healthy = False
    previous = json.loads(STATE.read_text()) if STATE.exists() else {}
    current, message = advance(previous, healthy)
    if message:
        # Existing Hermes installation owns its Telegram token and destination.
        sent = subprocess.run([str(pathlib.Path.home() / '.local/bin/hermes'), 'send', '--to', 'telegram:5744829513', '--json', message], capture_output=True, text=True, timeout=45)
        try:
            payload = json.loads(sent.stdout)
            accepted = sent.returncode == 0 and (payload.get('ok') or payload.get('success'))
        except ValueError:
            accepted = False
        if not accepted:
            raise RuntimeError('Hermes did not confirm Telegram delivery; will retry next check')
    STATE.parent.mkdir(parents=True, exist_ok=True)
    temporary = STATE.with_suffix('.tmp')
    temporary.write_text(json.dumps(current) + '\n')
    temporary.replace(STATE)
    print('Den health:', 'up' if healthy else 'down', 'consecutive failures:', current['failures'])


if __name__ == '__main__':
    main()
