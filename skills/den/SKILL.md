---
name: den
description: Post to, read from, and watch a Den chat server with the `den` CLI. Use when asked to send a message to the group, check what people said in a room, upload a clip, or react to chat events as they happen.
---

# Den

Den is a self-hosted chat for a small friend group. Everything goes through the `den` CLI. It reads `DEN_URL` (default `http://127.0.0.1:7000`) and `DEN_TOKEN` from the environment. Every command prints JSON.

Run `den --help` for the full list. Rooms can be named by `#name`, `name`, or their ULID. A DM is `@username`.

## What you will use most

```
den health                                 # is the server up
den channels                               # rooms you can see
den read general --limit 20                # recent messages, oldest first
den send general "hello"                   # post
den send general "agreed" --reply-to MSG_ID
den upload clips ./run.mp4                 # prints an upload id
den send clips "watch this" --upload UPLOAD_ID
den tail general                           # stream events as JSON lines until killed
den tail                                   # every room at once
den dm @bob                                # open a DM, then send to its id
```

Messages are markdown: `**bold**`, `_italic_`, `` `code` ``, fenced code blocks, links. Mention someone with `@username`.

## Reacting to events

`den tail` prints one JSON object per line. The ones that matter are `message_created` (fields: `id`, `channel_id`, `author_id`, `content`, `mention_ids`), `message_edited`, and `resync`. A `resync` means events may have been missed: run `den read` before acting on anything.

Ignore your own messages when replying to events, or you will talk to yourself.

## Manners

- You post as whichever identity the token belongs to. Bot tokens show a badge.
- Do not post more than the person asked for. One message beats three.
- Do not upload anything the person did not point you at.

## Shared canvases

```
den canvas create plans "raid layout"  # posts a card, prints the full object
den canvas get OBJECT_ID               # prints the record map only
den canvas patch OBJECT_ID --file patch.json
den canvas patch OBJECT_ID --file -    # read the patch from stdin
den canvas rename OBJECT_ID "north entrance"
```

Read the state before editing. `put` replaces each whole record by `id`; it does
not merge fields within a record. `remove` deletes record IDs. `base_version` is
informational, so concurrent edits to different records survive and the last
write to the same record wins. A patch must fit in 1 MiB and the complete state
in 8 MiB. Canvas creation returns `canvas_disabled` if the admin turned it off.
Existing canvases remain usable.

The renderer is pinned to tldraw 3.15.6. Send only document records, whose
`typeName` is `document`, `page`, `shape`, `binding`, or `asset`. Never patch
`instance`, camera, pointer, presence, or session records. Shape IDs start with
`shape:`. A shape's `parentId` is the page ID from `den canvas get`, or another
shape if deliberately grouping. Coordinates are page units, rotation is radians,
and opacity ranges from 0 to 1. `index` controls stacking, with `a1`, `a2`, `a3`
in ascending order. Use unique indices when adding to an existing canvas.
`black` is tldraw's theme-aware ink, so it draws light in Den's dark canvas.

This worked example seeds a new, empty canvas and draws a rectangle, an arrow,
and a text label. Save the JSON as `patch.json` and pass it to `den canvas patch`.
For an existing canvas, keep its document/page records and substitute its page
ID instead of replacing those records. Use unique shape IDs for each addition.

```json
{
  "base_version": 0,
  "put": [
    {"id":"document:document","typeName":"document","gridSize":10,"name":"","meta":{}},
    {"id":"page:page","typeName":"page","name":"Page 1","index":"a1","meta":{}},
    {
      "id":"shape:cover","typeName":"shape","type":"geo",
      "x":100,"y":100,"rotation":0,"index":"a1","parentId":"page:page",
      "isLocked":false,"opacity":1,"meta":{},
      "props":{
        "w":200,"h":120,"geo":"rectangle","dash":"draw","growY":0,
        "url":"","scale":1,"color":"black","labelColor":"black","fill":"none",
        "size":"m","font":"draw","align":"middle","verticalAlign":"middle",
        "richText":{"type":"doc","content":[{"type":"paragraph"}]}
      }
    },
    {
      "id":"shape:approach","typeName":"shape","type":"arrow",
      "x":340,"y":160,"rotation":0,"index":"a2","parentId":"page:page",
      "isLocked":false,"opacity":1,"meta":{},
      "props":{
        "kind":"arc","elbowMidPoint":0.5,"dash":"draw","size":"m","fill":"none",
        "color":"black","labelColor":"black","bend":0,"start":{"x":160,"y":0},
        "end":{"x":0,"y":0},"arrowheadStart":"none","arrowheadEnd":"arrow",
        "font":"draw","text":"","labelPosition":0.5,"scale":1
      }
    },
    {
      "id":"shape:label","typeName":"shape","type":"text",
      "x":100,"y":250,"rotation":0,"index":"a3","parentId":"page:page",
      "isLocked":false,"opacity":1,"meta":{},
      "props":{
        "color":"black","size":"m","font":"draw","textAlign":"start",
        "w":280,"scale":1,"autoSize":true,
        "richText":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Hold this corner"}]}]}
      }
    }
  ],
  "remove": []
}
```

Geo shapes and text labels use `richText`. In this pinned version arrow labels
still use `text`. An arrow's `start` and `end` are relative to its `x`, `y`.
To move a record, read it, change `x` and `y`, and put the complete record back.
To remove the arrow, send `{"base_version":0,"put":[],"remove":["shape:approach"]}`.
`object_patched` events appear in `den tail`; refetch on `resync`.


## Machines and terminals

Machines belong to people. Ask for permission before acting on another person's
machine. A chat card or a host ID alone is not permission to view its terminal.

```bash
den host list
den host enroll
den access request HOST_ID control --minutes 60
den tail
# Wait for access_decided with status allowed and your request ID.
den access grants
den terminal open HOST_ID --in general
# The owner must promote you to controller before input is accepted.
den terminal write SESSION_ID 'printf "hello\n"
'
den access revoke GRANT_ID
```

Use `view` for watching, `control` for opening and typing. Requests default to
one hour. Use `--standing` only when the owner has asked for ongoing access.
Requests appear in the owner's DM, including the bot badge for agent identities.
Wait for the decision event, then act. If checking with REST, never poll faster
than every five seconds. A grant does not take control from the current operator;
request control and wait for the owner to promote you. Stop on denial, expiry,
or revocation. The owner can answer through the chat card or
`den access decide REQUEST_ID --allow`; omitting `--allow` denies it.
