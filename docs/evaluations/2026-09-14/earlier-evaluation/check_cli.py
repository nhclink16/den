"""Black-box acceptance checks for the assigned den read --all contract."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

CID = '00000000000000000000000100'
UID = '00000000000000000000000200'

def mid(n):
    return f'{n:026d}'

def message(n):
    return dict(id=mid(n), channel_id=CID, author_id=UID, content=f'message {n}',
                reply_to=None, created_at='2026-09-14T00:00:00Z', edited_at=None,
                attachments=[], objects=[], reactions=[], mention_ids=[])

def case(binary, name, args, expected=None, *, count=7, fault=None, no_requests=False,
         name_resolution=False, dm_resolution=False, one_page=False):
    requests = []
    page_calls = []
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass
        def send_json(self, code, value):
            body = json.dumps(value).encode()
            self.send_response(code)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        def do_POST(self):
            requests.append(('POST', self.path))
            self.rfile.read(int(self.headers.get('Content-Length', 0)))
            if self.path == '/dms':
                return self.send_json(200, dict(id=CID, name='dm', category_id=None, kind='dm', position=0, member_ids=[UID]))
            self.send_json(404, {'error':'not_found','message':'fixture path'})
        def do_GET(self):
            requests.append(('GET', self.path))
            parsed = urlparse(self.path)
            if self.headers.get('Authorization') != 'Bearer synthetic-token':
                return self.send_json(401, {'error':'unauthorized','message':'fixture auth'})
            if parsed.path == '/channels':
                return self.send_json(200, [dict(id=CID, name='general', category_id=None, kind='text', position=0, member_ids=[])])
            if parsed.path == '/users':
                return self.send_json(200, [dict(id=UID, username='bob', display_name='Bob', role='member', bot=False, avatar_url=None)])
            if parsed.path != f'/channels/{CID}/messages':
                return self.send_json(404, {'error':'not_found','message':'fixture path'})
            q = parse_qs(parsed.query)
            page_calls.append(q)
            if fault == 'later-error' and len(page_calls) > 1:
                return self.send_json(503, {'error':'unavailable','message':'fixture later-page failure'})
            if len(page_calls) > 8:
                return self.send_json(500, {'error':'loop','message':'fixture guard'})
            limit = int(q.get('limit', ['50'])[0])
            if not 1 <= limit <= 200 or ('before' in q and 'after' in q):
                return self.send_json(400, {'error':'bad_request','message':'invalid query'})
            ids = list(range(1, count+1))
            if fault == 'stuck':
                ids = ids[-limit:]
            elif 'after' in q:
                ids = [n for n in ids if mid(n) > q['after'][0]][:limit]
            else:
                if 'before' in q:
                    ids = [n for n in ids if mid(n) < q['before'][0]]
                ids = ids[-limit:]
            self.send_json(200, [message(n) for n in ids])
    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    env = os.environ.copy()
    for key in list(env):
        if key.startswith('DEN_'):
            env.pop(key)
    with tempfile.TemporaryDirectory(prefix='den-cli-check-') as tmp:
        cmd = [str(binary), '--url', f'http://127.0.0.1:{server.server_port}', '--token', 'synthetic-token', '--config', str(Path(tmp)/'missing.json'), 'read', *args]
        try:
            p = subprocess.run(cmd, capture_output=True, text=True, env=env, timeout=8)
            error = None
            if expected is not None:
                try:
                    got = [m['id'] for m in json.loads(p.stdout)]
                except (ValueError, TypeError, KeyError):
                    got = None
                ok = p.returncode == 0 and got == [mid(n) for n in expected]
            else:
                got = None
                ok = p.returncode != 0 and not p.stdout.strip()
            if no_requests:
                ok = ok and not requests
            if one_page:
                ok = ok and len(page_calls) == 1
            if fault == 'stuck':
                ok = ok and len(page_calls) <= 3
            if name_resolution:
                ok = ok and sum(path == '/channels' for _,path in requests) == 1
            if dm_resolution:
                ok = ok and sum(path == '/users' for _,path in requests) == 1 and sum(method == 'POST' and path == '/dms' for method,path in requests) == 1
            result = dict(name=name, passed=bool(ok), returncode=p.returncode, requests=requests, stdout=p.stdout[:1500], stderr=p.stderr[:1500], actual_ids=got)
        except subprocess.TimeoutExpired:
            result = dict(name=name, passed=False, error='timed out', requests=requests)
    server.shutdown()
    server.server_close()
    return result

def check(binary):
    cases = [
        ('legacy_latest', [CID,'--limit','3'], [5,6,7], {'one_page':True}),
        ('legacy_before', [CID,'--before',mid(6),'--limit','3'], [3,4,5], {'one_page':True}),
        ('legacy_after', [CID,'--after',mid(2),'--limit','3'], [3,4,5], {'one_page':True}),
        ('all_backwards', [CID,'--all','--limit','3'], list(range(1,8)), {}),
        ('all_before', [CID,'--all','--before',mid(6),'--limit','3'], list(range(1,6)), {}),
        ('all_after', [CID,'--all','--after',mid(2),'--limit','3'], list(range(3,8)), {}),
        ('all_empty', [CID,'--all','--limit','3'], [], {'count':0}),
        ('all_exact_multiple', [CID,'--all','--limit','3'], list(range(1,7)), {'count':6}),
        ('all_single_item_pages', [CID,'--all','--limit','1'], [1,2,3], {'count':3}),
        ('all_later_error', [CID,'--all','--limit','3'], None, {'fault':'later-error'}),
        ('all_no_progress', [CID,'--all','--limit','3'], None, {'fault':'stuck'}),
        ('resolve_name_once', ['general','--all','--limit','3'], list(range(1,8)), {'name_resolution':True}),
        ('resolve_dm_once', ['@bob','--all','--limit','3'], list(range(1,8)), {'dm_resolution':True}),
    ]
    for all_flag in [[], ['--all']]:
        suffix = '_all' if all_flag else '_legacy'
        for value in ['0','201']:
            cases.append(('reject_limit_'+value+suffix, ['general',*all_flag,'--limit',value], None, {'no_requests':True}))
        cases.append(('reject_both_cursors'+suffix, ['general',*all_flag,'--before',mid(6),'--after',mid(2)], None, {'no_requests':True}))
    return [case(binary, name, args, expected, **options) for name,args,expected,options in cases]

if __name__ == '__main__':
    import sys
    print(json.dumps(check(Path(sys.argv[1])), indent=2))
