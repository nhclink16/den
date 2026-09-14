# M2 server notes

The requested server work is complete. `apps/web` was not edited. Migration `0002_web.sql` runs on startup and indexes existing messages for search and mentions. The schema exported by `GET /openapi.json` derives from `den-core`.

## Run

```bash
cargo build --workspace
DEN_WEB_DIR=/absolute/path/to/apps/web/dist ./target/debug/den-server
```

`DEN_WEB_DIR` defaults to `apps/web/dist`, relative to the working directory. The server serves its files at `/`, with `index.html` fallback for extensionless client routes such as `/inbox` or `/chat/ID`. API prefixes remain reserved, including `/channels`, `/users`, `/messages`, `/uploads`, `/search`, and `/auth`. Missing assets and unknown API routes return errors. A missing build directory leaves the API usable. No separate static-file server is required.

M1 authentication, configuration, and CLI commands still apply. Restart the server after updating it.

## Client contract

| Method and path | Request / response |
| --- | --- |
| `GET /messages/{id}` | Authorized message lookup, including reply parents outside the loaded page. |
| `PUT /messages/{id}/reactions` | `{ "emoji": "👍" }`; idempotently adds your reaction. Returns `Reaction[]`. |
| `DELETE /messages/{id}/reactions` | Same JSON body; removes only your reaction. Returns `Reaction[]`. |
| `GET /users/me/read-state` | `ChannelReadState[]` for visible channels. |
| `PUT /channels/{id}/read` | `{ "message_id": "ULID" }`; returns that channel's updated read state. |
| `GET/PUT /users/me/notification-preferences` | `{ "mentions": true, "dms": true, "subscribed_channel_ids": [] }`. PUT replaces the preferences. |
| `GET /search/messages` | `q`, optional `channel_id`, `before`, `limit`. Returns `Message[]`, newest first. |
| `GET /presence` | `{ "online_user_ids": ["ULID"] }`. |
| `GET/HEAD /uploads/{id}/thumbnail` | Authenticated PNG preview, or 404 if unavailable. |

Messages now include `reactions: [{ emoji, user_ids }]` and `mention_ids: Id[]`. Replies continue to use `reply_to`; cross-channel parents are rejected and deleted parents become null.

Read state contains `channel_id`, nullable `last_read_id`, `unread_count`, `mention_count`, and `notification_count`. Fetching messages does not mark them read. Explicit read markers advance monotonically and survive deletion of the marked message. Counts exclude your own messages. Unread history includes messages before the first marker.

Notifications default to mentions and DMs; public channels require an explicit subscription for ordinary messages. All preferences can be disabled. Multiple matching reasons produce one notification, with mention taking precedence over DM, then subscription. Editing messages recalculates mentions and unread counts without sending a new alert. Mentions use visible `@username` text, case-insensitively, excluding code and email addresses; unknown users and outsiders to a DM are not mentioned.

Search uses SQLite FTS5 and treats whitespace-separated terms literally with AND semantics. Queries allow 1 to 200 bytes, at most 20 terms, and limits of 1 to 100, default 50. Edits/deletes update the index. Search, replies, reactions, thumbnails, and unread state all enforce DM membership, including for admins.

## Live events

Send `{"type":"typing","channel_id":"ULID"}` over the authenticated WebSocket. The server supplies the user identity and limits typing broadcasts to one per channel per socket every two seconds. Clear typing indicators after five seconds without an update. Unknown client event shapes close the connection.

New server event tags are `reactions_updated`, `read_state_updated`, `notification_preferences_updated`, and `notification`. Their exact payloads are in the shared `Event` schema. Read-state/preferences updates and notifications go only to that user's sockets. Notifications carry the message and a `mention`, `dm`, or `subscribed_channel` reason. They are live alerts, not a durable delivery queue; unread counts persist.

`presence` events report first connection and last disconnection per user. Multiple tabs count as one online user. Heartbeat timeouts also clear presence. On the initial `resync` or any reconnect, refetch messages, read state, preferences, and `/presence`. There is no durable event replay.

## Thumbnails and checks

Completed PNG, JPEG, GIF, and WebP uploads get PNG thumbnails, at most 512 by 512, preserving aspect ratio and applying image orientation. `Upload.thumbnail_url` is nullable. Animated images use the first frame. Processing uses one worker, a 32 MiB input cap, dimension/pixel limits, and a 64 MiB decoder allocation limit. Unsupported or malformed images remain downloadable without a preview. Old uploads and missing derived files can generate previews on demand through the thumbnail endpoint.

Passed: 12 integration tests, including an M1 database upgrade, privacy checks, tab-aware presence, decoded thumbnail pixels, and SPA fallback; workspace tests, formatting, Clippy with warnings denied, and SQLx metadata verification. The M1 CLI smoke test still passes chat, reconnect, the 41-second clip, and bot-token flows. A running binary also passed a custom `DEN_WEB_DIR` and M2 OpenAPI check.

Stopped after the requested server scope. Deployment and client work remain separate.
