# M1 run and API notes

M1 passes its acceptance checks. The server and CLI support text channels, private DMs and group DMs, messages, invites, sessions, bot identities, bearer tokens, resumable uploads, and a shared WebSocket event stream.

## Run locally

From the repository root, in the server terminal:

```bash
cargo build --workspace
./target/debug/den-server
```

In another terminal, initialize the empty instance:

```bash
export DEN_CONFIG=/tmp/den-alice.json
read -rsp 'Password, at least 12 characters: ' DEN_PASSWORD
export DEN_PASSWORD
./target/debug/den init alice
unset DEN_PASSWORD
./target/debug/den channels
./target/debug/den invite create --uses 1 --hours 24
```

`init` reads the server's local `data/bootstrap.key`, which disappears after successful initialization. Subsequent starts preserve accounts. Passwords can also arrive through `--password-stdin`. Usernames use 3 to 32 lowercase letters, digits, or underscores.

For another account, use a separate `DEN_CONFIG`, set its password as above, and run `den register bob --invite CODE`. Later use `den login bob`. Examples below abbreviate `./target/debug/den` to `den`:

```bash
den send CHANNEL_ID 'hello'
den read CHANNEL_ID --limit 50
den tail CHANNEL_ID
den dm OTHER_USER_ID
den upload CHANNEL_ID clip.mp4
den send CHANNEL_ID 'clip attached' --upload UPLOAD_ID
den bot create clanker
den token create replacement --user BOT_USER_ID
```

Bot creation prints the bot and its token once. Set `DEN_TOKEN` in the agent process to post as that bot. `den token list` and `den token revoke TOKEN_ID` manage your credentials and those of bots you own. Results are JSON; upload progress and reconnect warnings go to stderr. `den --help` lists administrative CRUD commands.

## API contract

`GET /openapi.json` exports schemas derived from `den-core`, including `Event`, authentication schemes, and request/response types.

| Resource | Routes |
| --- | --- |
| Auth | `POST /auth/init`, `/auth/register`, `/auth/login`, `/auth/logout` |
| Users | `GET /users`, `/users/me`; `POST /bots` |
| Credentials | `POST /invites`, `DELETE /invites/{id}`; `GET/POST /tokens`, `DELETE /tokens/{id}` |
| Channels | `GET/POST /channels`, `GET/PUT/DELETE /channels/{id}`; `POST /dms` |
| Categories | `GET/POST /categories`, `PUT/DELETE /categories/{id}` |
| Messages | `GET/POST /channels/{id}/messages`, `PATCH/DELETE /messages/{id}` |
| Uploads | `POST /uploads`, `GET/PATCH /uploads/{id}`, `POST /uploads/{id}/complete`, `GET/HEAD /uploads/{id}/file` |

CLI/agents send `Authorization: Bearer TOKEN`. Browsers use the session cookie; writes also require the configured `Origin` and an `X-CSRF-Token` header containing the login JSON's `csrf_token`. Sessions expire after 90 days. Logout revokes the current credential. HTTPS cookies are Secure; HTTP development is restricted to loopback origins.

IDs are server-generated ULIDs. Message lists return ascending IDs, with `before` or `after` cursors and limits of 1 to 200. DMs expose `member_ids` only to participants. Tokens accept optional `user_id` for an owned bot. Private content returns 404 to outsiders, including admins.

`/ws` starts with `{"type":"resync","reason":"..."}`. Refetch state after connecting or reconnecting; there is no event replay. Message events carry their fields beside the `type` tag. Slow consumers disconnect and resync. The server rechecks credentials before events and every five seconds, closing revoked connections. Typing/presence variants are reserved for M2.

Uploads accept chunks up to 8 MiB with `Upload-Offset`; GET status supplies the committed offset. Resume the original unchanged file with `den upload CHANNEL_ID FILE --resume UPLOAD_ID`. Finalization is retryable. Files require channel access and support byte ranges. Incomplete uploads expire after 24 hours; completed files remain. The server reserves 64 MiB of disk headroom plus pending upload bytes.

## Configuration and verification

Defaults: `DEN_BIND=127.0.0.1:7000`, `DEN_DB=data/den.db`, `DEN_UPLOADS=data/uploads`, `DEN_BOOTSTRAP_FILE=data/bootstrap.key`, `DEN_MAX_UPLOAD_BYTES=1073741824`. Set `DEN_ORIGIN` to the public HTTPS origin behind a proxy. CLI `DEN_URL` selects the server; credentials default to `$XDG_CONFIG_HOME/den/credentials.json` or `~/.config/den/credentials.json`.

Passed locally: formatting, Clippy with warnings denied, five API integration tests, and SQLx metadata verification. `cargo build --workspace && python3 scripts/m1-smoke.py` repeats the acceptance check using disposable data and ffmpeg: two CLI sessions exchange messages, reconnect after restart, upload and decode an 11,894,863-byte 41-second H.264/AAC clip over authenticated HTTP, and post with a bot token.

Builds use committed `.sqlx` metadata. After changing queries, migrate a development database and run `SQLX_OFFLINE=false DATABASE_URL=sqlite://PATH cargo sqlx prepare --workspace -- --all-targets` with sqlx-cli 0.8.6. CI checks metadata against the migrations.

Web UI, agent skill documentation, and Hermes integration remain with their assigned milestones/owners. No M2 work started.
