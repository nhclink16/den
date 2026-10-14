"""Seed a fresh Den server with a made-up friend group for the Den Shell UI check.

Everyone and everything here is invented. The UI test signs in as ellis.
  python3 seed.py http://127.0.0.1:7000 <data>/bootstrap.key
"""
import json, sys, urllib.request

BASE = sys.argv[1]
BOOT = open(sys.argv[2]).read().strip()
PW = 'correct horse battery'


def req(method, path, body=None, tok=None):
    r = urllib.request.Request(BASE + path, method=method, data=json.dumps(body).encode() if body is not None else None,
                               headers={'content-type': 'application/json', 'origin': BASE,
                                        **({'authorization': f'Bearer {tok}'} if tok else {})})
    with urllib.request.urlopen(r) as resp:
        t = resp.read()
        return json.loads(t) if t else None


toks = {'rowan': req('POST', '/auth/init', {'username': 'rowan', 'password': PW, 'bootstrap_token': BOOT,
                                             'display_name': 'Rowan'})['token']}
admin = toks['rowan']
for u, d in [('maya', 'Maya'), ('theo', 'Theo'), ('juno', 'Juno'), ('sam', 'Sam Okafor'), ('ellis', 'Ellis')]:
    inv = req('POST', '/invites', {'uses': 1, 'expires_in_hours': 24}, admin)['code']
    toks[u] = req('POST', '/auth/register', {'username': u, 'password': PW, 'invite': inv, 'display_name': d})['token']
ids = {u: req('GET', '/users/me', tok=t)['id'] for u, t in toks.items()}
for u, (e, t) in {'maya': ('🎮', 'grinding the end'), 'sam': ('😴', 'night shift')}.items():
    req('PATCH', '/users/me/profile', {'status': {'emoji': e, 'text': t}}, toks[u])

chans = {c['name']: c for c in req('GET', '/channels', tok=admin)}
games = req('POST', '/categories', {'name': 'Games', 'position': 1}, admin)
hangs = req('POST', '/categories', {'name': 'Hangs', 'position': 2}, admin)
for i, (n, cat) in enumerate([('minecraft', games), ('plans', hangs), ('music', hangs)]):
    chans[n] = req('POST', '/channels', {'name': n, 'category_id': cat['id'], 'position': i + 2}, admin)


def say(ch, who, text, reply=None):
    return req('POST', f"/channels/{chans[ch]['id']}/messages", {'content': text, 'reply_to': reply}, toks[who])


for who, text in [
    ('maya', 'morning all'), ('theo', 'is it though'), ('juno', 'coffee machine at work exploded'),
    ('sam', 'exploded how'), ('juno', 'like the steam thing. nobody hurt, just loud'),
    ('rowan', 'anyone around tonight for a few rounds?'), ('theo', 'after 9 maybe'), ('ellis', "I'm in after dinner"),
    ('maya', 'what are we even playing'), ('rowan', 'minecraft, the nether hub needs finishing'),
    ('theo', 'I am NOT doing the ice road again'), ('ellis', 'I can do the ice road if someone brings food'),
]:
    say('general', who, text)
plan = say('general', 'rowan', 'Saturday: cabin trip is ON. Leaving 10am from my place. Reply here if you need a ride.')
for who, emoji in [('maya', '🎉'), ('theo', '🎉'), ('juno', '👍')]:
    req('PUT', f"/messages/{plan['id']}/reactions", {'emoji': emoji}, toks[who])
say('general', 'juno', 'I need a ride, my car is in the shop', reply=plan['id'])
say('general', 'sam', 'me too, can bring snacks', reply=plan['id'])
say('general', 'maya', '@ellis are you bringing the board games saturday?')
say('minecraft', 'theo', 'two stacks of diamonds. gone. lava.')
say('plans', 'rowan', 'cabin list: sleeping bags, the good speaker, board games, ice')
say('music', 'theo', 'queue is open tonight, put something on')
dm = req('POST', '/dms', {'member_ids': [ids['ellis']]}, toks['maya'])
req('POST', f"/channels/{dm['id']}/messages", {'content': 'also do you still have my controller'}, toks['maya'])
print('seeded')
