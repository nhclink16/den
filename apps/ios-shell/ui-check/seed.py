"""Seed a fresh Den server with a made-up friend group for the Den Shell UI check and the visual tour.

Everyone and everything here is invented, pictures included (media/, drawn, not photographed). The UI test signs in as
ellis. Enough is here for every tour stop to look lived-in: profile pictures, pictures and a GIF, reactions, a thread,
a bot, someone with a long name, categories, an empty room and unread counts.
  python3 seed.py http://127.0.0.1:7000 <data>/bootstrap.key
"""
import json, os, sys, urllib.request

BASE = sys.argv[1]
BOOT = open(sys.argv[2]).read().strip()
PW = 'correct horse battery'
MEDIA = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'media')
TYPES = {'.jpg': 'image/jpeg', '.gif': 'image/gif'}


def req(method, path, body=None, tok=None, raw=None, headers=None):
    data = raw if raw is not None else json.dumps(body).encode() if body is not None else None
    r = urllib.request.Request(BASE + path, method=method, data=data,
                               headers={'content-type': 'application/json', 'origin': BASE,
                                        **({'authorization': f'Bearer {tok}'} if tok else {}), **(headers or {})})
    with urllib.request.urlopen(r) as resp:
        t = resp.read()
        return json.loads(t) if t else None


def media(name):
    return open(os.path.join(MEDIA, name), 'rb').read(), TYPES[os.path.splitext(name)[1]]


toks = {'rowan': req('POST', '/auth/init', {'username': 'rowan', 'password': PW, 'bootstrap_token': BOOT,
                                             'display_name': 'Rowan'})['token']}
admin = toks['rowan']
for u, d in [('maya', 'Maya'), ('theo', 'Theo'), ('juno', 'Juno'), ('sam', 'Sam Okafor'), ('ellis', 'Ellis'),
             ('bart', 'Bartholomew Featherstonehaugh-Whitley')]:
    inv = req('POST', '/invites', {'uses': 1, 'expires_in_hours': 24}, admin)['code']
    toks[u] = req('POST', '/auth/register', {'username': u, 'password': PW, 'invite': inv, 'display_name': d})['token']
bot = req('POST', '/bots', {'username': 'pebble', 'display_name': 'Pebble'}, admin)
toks['pebble'] = bot['credential']['token']
ids = {u: req('GET', '/users/me', tok=t)['id'] for u, t in toks.items()}

for u, profile in {
    'maya': {'status': {'emoji': '🎮', 'text': 'grinding the end'}, 'bio': 'Builds the redstone. Breaks the redstone.'},
    'sam': {'status': {'emoji': '😴', 'text': 'night shift'}},
    'ellis': {'status': {'emoji': '☕', 'text': 'back after lunch'}, 'bio': 'Ice road engineer. Brings the board games.'},
    'pebble': {'bio': 'Rowan\'s helper. Posts server backups and reminders.'},
}.items():
    req('PATCH', '/users/me/profile', profile, toks[u])
for u in ['maya', 'theo', 'juno', 'ellis', 'bart']:
    data, kind = media(f'avatar-{u}.jpg')
    req('PUT', '/users/me/avatar', tok=toks[u], raw=data, headers={'content-type': kind})
data, kind = media('avatar-bot.jpg')
req('PUT', f"/users/{ids['pebble']}/avatar", tok=admin, raw=data, headers={'content-type': kind})
data, kind = media('banner-maya.jpg')
req('PUT', '/users/me/banner', tok=toks['maya'], raw=data, headers={'content-type': kind})

chans = {c['name']: c for c in req('GET', '/channels', tok=admin)}
games = req('POST', '/categories', {'name': 'Games', 'position': 1}, admin)
hangs = req('POST', '/categories', {'name': 'Hangs', 'position': 2}, admin)
for i, (n, cat) in enumerate([('minecraft', games), ('plans', hangs), ('music', hangs), ('books', hangs)]):
    chans[n] = req('POST', '/channels', {'name': n, 'category_id': cat['id'], 'position': i + 2}, admin)


def upload(ch, who, name):
    data, kind = media(name)
    up = req('POST', '/uploads', {'channel_id': ch, 'filename': name, 'content_type': kind, 'size': len(data)}, toks[who])
    req('PATCH', f"/uploads/{up['id']}", tok=toks[who], raw=data,
        headers={'content-type': 'application/offset+octet-stream', 'upload-offset': '0'})
    return req('POST', f"/uploads/{up['id']}/complete", tok=toks[who])['id']


def say(ch, who, text, reply=None, picture=None):
    ch = chans[ch]['id'] if ch in chans else ch
    uploads = [upload(ch, who, picture)] if picture else []
    return req('POST', f'/channels/{ch}/messages', {'content': text, 'reply_to': reply, 'upload_ids': uploads}, toks[who])


def react(message, *pairs):
    for who, emoji in pairs:
        req('PUT', f"/messages/{message['id']}/reactions", {'emoji': emoji}, toks[who])


def read(ch, who, message):
    ch = chans[ch]['id'] if ch in chans else ch
    req('PUT', f'/channels/{ch}/read', {'message_id': message['id'], 'roots_only': True}, toks[who])


for who, text in [
    ('maya', 'morning all'), ('theo', 'is it though'),
    ('juno', 'coffee machine at work exploded. Not like a little pop either, the whole steam wand thing went off '
             'while Priya was frothing milk and now there is a brown stain on the ceiling that nobody can reach'),
    ('sam', 'exploded how'), ('juno', 'like the steam thing. nobody hurt, just loud'),
    ('bart', "Sorry, late to this. Is anyone else's train stuck outside the station or is it just mine again?"),
]:
    say('general', who, text)
dog = say('general', 'juno', 'meanwhile at home. he has not moved in three hours', picture='dog.jpg')
react(dog, ('maya', '😂'), ('theo', '😂'), ('sam', '😂'), ('ellis', '❤️'), ('rowan', '❤️'))
for who, text in [('rowan', 'anyone around tonight for a few rounds?'), ('theo', 'after 9 maybe'),
                  ('ellis', "I'm in after dinner"), ('maya', 'what are we even playing')]:
    say('general', who, text)
say('general', 'rowan', 'minecraft, the nether hub needs finishing', picture='nether.jpg')
say('general', 'theo', 'I am NOT doing the ice road again')
lava = say('general', 'theo', '', picture='lava.gif')
react(lava, ('juno', '💀'), ('maya', '💀'))
ice = say('general', 'ellis', 'I can do the ice road if someone brings food')
say('general', 'sam', 'found the cabin listing again: https://example.com/cabins/lakeside-12 (hot tub confirmed)')
say('general', 'pebble', 'Nightly backup finished: minecraft world 2.1 GB, took 14 s. Next one at 4:00.')
plan = say('general', 'rowan', 'Saturday: cabin trip is ON. Leaving 10am from my place. Reply here if you need a ride.')
react(plan, ('maya', '🎉'), ('theo', '🎉'), ('juno', '👍'), ('bart', '🎉'))
say('general', 'juno', 'I need a ride, my car is in the shop', reply=plan['id'])
say('general', 'sam', 'me too, can bring snacks', reply=plan['id'])
say('general', 'maya', "I'll drive. Room for three plus snacks", reply=plan['id'])
say('general', 'maya', '@ellis are you bringing the board games saturday?')
read('general', 'ellis', ice)

mc = [say('minecraft', 'theo', 'two stacks of diamonds. gone. lava.'),
      say('minecraft', 'maya', 'rip. did you at least get the elytra back'),
      say('minecraft', 'theo', 'the elytra was in the lava too')]
read('minecraft', 'ellis', mc[-1])
say('plans', 'rowan', 'cabin list: sleeping bags, the good speaker, board games, ice')
say('plans', 'maya', 'this is the view from the dock apparently', picture='lake.jpg')
say('plans', 'juno', 'ok I am sold')
say('music', 'theo', 'queue is open tonight, put something on')
req('PUT', '/users/me/notification-preferences',
    {'mentions': True, 'dms': True, 'subscribed_channel_ids': [chans['plans']['id']]}, toks['ellis'])

dm = req('POST', '/dms', {'member_ids': [ids['ellis']]}, toks['maya'])
req('POST', f"/channels/{dm['id']}/messages", {'content': 'also do you still have my controller'}, toks['maya'])
theo = req('POST', '/dms', {'member_ids': [ids['ellis']]}, toks['theo'])
for who, text in [('theo', 'can you send me the seed for the new world'), ('ellis', '-4172093857, spawn is by the mesa'),
                  ('theo', 'legend')]:
    last = say(theo['id'], who, text)
read(theo['id'], 'ellis', last)
print('seeded')
