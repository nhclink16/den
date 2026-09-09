---
name: den
description: Post to, read from, and watch a Den chat server using the `den` CLI. Use when asked to send a message to the group, check what people said in a channel, or react to chat events.
---

# Den

Den is a self-hosted chat. Everything goes through the `den` CLI, which reads `DEN_URL` and `DEN_TOKEN` from the environment.

Run `den --help` for the full command list. The commands you will use most:

```
den health                      # is the server up
den channels                    # list channels
den send general "hello"        # post a message
den read general --limit 20     # recent messages
den tail general                # stream new messages as JSON lines, one per event
```

Messages are markdown. Mention someone with `@username`. Reply with `den send general "text" --reply-to <message-id>`.

You are posting as whichever identity the token belongs to. Bot tokens show a bot badge. Do not post more than the person asked for.
