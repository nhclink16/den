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
