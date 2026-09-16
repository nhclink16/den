"""Local-only Den CLI incident reproduction. Requires supplied arena Python runtime."""
import argparse, json, logging, os, subprocess, tempfile, threading, time
from pathlib import Path
from http import HTTPStatus
from websockets.sync.server import serve

TOKEN='synthetic-arena-token'
CID='00000000000000000000000100'
OTHER='00000000000000000000000200'

def event(i, channel=CID):
    return {'type':'message_created','id':f'{i:026d}','channel_id':channel,'author_id':'00000000000000000000000300','content':f'event {i}','reply_to':None,'created_at':'2026-09-14T00:00:00Z','edited_at':None,'attachments':[],'objects':[],'reactions':[],'mention_ids':[]}

def exercise(binary, *, mode='reconnect', channel=False, deadline=8):
    attempts=[]; accepted=[]; pong=[]; request_paths=[]
    def process(connection, request):
        request_paths.append(request.path)
        if request.path.startswith('/channels/'):
            value={'id':CID,'name':'general','kind':'text','category_id':None,'position':0,'member_ids':[]}
            response=connection.respond(HTTPStatus.OK,json.dumps(value));response.headers['Content-Type']='application/json';return response
        auth=request.headers.get('Authorization')=='Bearer '+TOKEN
        attempts.append(auth); number=len(attempts)
        if mode in ['401','403']:
            return connection.respond(int(mode),'Credentials rejected\n')
        if mode=='transient' and number==1:
            return connection.respond(HTTPStatus.SERVICE_UNAVAILABLE,'Retry later\n')
        if not auth or number>=3:
            return connection.respond(HTTPStatus.UNAUTHORIZED,'Credentials rejected\n')
        return None
    def handler(ws):
        number=len(attempts); accepted.append(number)
        ws.send(json.dumps({'type':'resync','reason':'connected; refresh history'}))
        if mode=='filter':
            ws.send(json.dumps(event(90,OTHER)))
            ws.send(json.dumps({'type':'presence','user_id':'00000000000000000000000300','online':True}))
        waiter=ws.ping(b'arena-ping');pong.append(waiter.wait(2))
        ws.send(json.dumps(event(number)))
        ws.close()
    logging.getLogger('websockets.server').setLevel(logging.CRITICAL)
    with serve(handler,'127.0.0.1',0,process_request=process,ping_interval=None,close_timeout=.3) as server:
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        env={k:v for k,v in os.environ.items() if not k.startswith('DEN_')}
        with tempfile.TemporaryDirectory(prefix='arena-tail-') as tmp:
            cmd=[str(binary),'--url',f'http://127.0.0.1:{server.socket.getsockname()[1]}','--token',TOKEN,'--config',str(Path(tmp)/'credentials.json'),'tail']
            if channel:cmd.append(CID)
            start=time.monotonic();p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=env)
            timeout=False
            try: stdout,stderr=p.communicate(timeout=deadline)
            except subprocess.TimeoutExpired:
                timeout=True;p.kill();stdout,stderr=p.communicate()
        server.shutdown();thread.join(timeout=2)
    try:events=[json.loads(line) for line in stdout.splitlines()]
    except ValueError:events=None
    ids=[e['id'] for e in events or [] if e['type']=='message_created']
    expected=[] if mode in ['401','403'] else ([f'{2:026d}'] if mode=='transient' else [f'{1:026d}',f'{2:026d}'])
    checks={'exits_on_auth_rejection':not timeout and p.returncode!=0,
            'one_connection_on_initial_rejection':len(attempts)==1 if mode in ['401','403'] else True,
            'auth_on_every_connection':all(attempts),
            'expected_events_once':ids==expected,
            'json_only_stdout':events is not None,
            'ping_response':all(pong),
            'gap_diagnostics':('Gap:' in stderr and 'Connection lost' in stderr) if accepted else True,
            'global_resync_events':sum(e['type']=='resync' for e in events or [])==len(accepted),
            'global_presence_events':sum(e['type']=='presence' for e in events or [])==len(accepted) if mode=='filter' else True}
    return {'passed':all(checks.values()),'checks':checks,'elapsed_seconds':round(time.monotonic()-start,2),'attempts_authenticated':attempts,'accepted_connections':accepted,'stdout':stdout,'stderr':stderr,'returncode':p.returncode,'timed_out':timeout,'requests':request_paths}

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('binary',type=Path);args=parser.parse_args()
    result=exercise(args.binary.resolve());print(json.dumps(result,indent=2));raise SystemExit(0 if result['passed'] else 1)
