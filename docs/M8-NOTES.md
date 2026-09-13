# M8 portable

Item 1 is complete. Export/import and instance name are next.

## Public release preparation

Nicholas authorized making the repository public after a sensitive-data audit.
Gitleaks 8.30.1, downloaded with its verified upstream checksum, scanned all Git
history with no findings. A separate exact-value check compared 541 historical
blobs against the current development and production Den credentials without
printing those values. It found no matches. Screenshot versions were OCR-scanned
and the terminal/DM examples inspected; they contain labeled smoke data and
machine names, not credentials. Audit reports stay outside the repository.
Tracked deployment files contain public endpoints, tailnet addresses and local
paths. No private keys, databases or credential files are tracked. Commit authors
use a GitHub noreply address.

The repository is now public. Actions jobs started immediately after the change.

The agreed split is now explicit in package metadata and LICENSE: AGPL-3.0-only
for den-server and den-host, MIT for original client, CLI, shared types and plugin
code. Third-party notices, including tldraw's separate license, remain in place.
Earlier MIT versions retain their permissions.

Before the visibility change, GitHub check 103792472606 could not start because
of account payments or the spending limit. Both native acceptance machines were
reachable: Nicholas-Work.local as nicholascaron, Apple arm64, and nicholas as the
Windows x86_64 PC. The VPS identified itself as vps-2fd9743a with Den active.

## 1. Binaries and installer

[v0.1.0](https://github.com/nhclink16/den/releases/tag/v0.1.0) was built and
published by [Actions run 34788677367](https://github.com/nhclink16/den/actions/runs/34788677367).
All three native jobs passed. The Linux bundle includes server, CLI, host, SPA,
deployment scripts and licenses; macOS arm64 and Windows x86_64 provide CLI and
host binaries. All eleven downloaded release files matched SHA256SUMS, including
the matching tagged source. The local fallback used the same package script with
`CARGO_TARGET_DIR=/mnt/storage/den-m8-target`; outputs remain outside the checkout.
The packaging helper now copies only committed deployment files, excluding any
ignored local secrets or data.

The VPS backup completed before `bash deploy/release.sh v0.1.0`. That deployment
downloaded and checked the bundle without compiling. Public health passed and
both installer paths returned their exact script bodies as text, including HEAD.
Production serves them from `/opt/den/deploy`, set through the systemd unit.

Settings, Machines offers the curl command and a keyboard-operable Windows
PowerShell checkbox. `install-host.sh` resolves latest once, checks the matching
SHA-256, installs under `~/.local/bin`, enrolls, and installs the user service.
The PowerShell twin installs under `%LOCALAPPDATA%\den`. Its first real run caught
a PowerShell 7 byte-response issue; commit `43a5507` reads downloaded checksums as
UTF-8 and updates the served script. The v0.1.0 bundle predates that script fix;
use the current website installer or checkout when provisioning Windows.

Acceptance used the public downloads and public Den, with no Rust installation:

| Machine | Installer/service | Browser keyboard to PTY to Ghostty |
| --- | --- | --- |
| Clean Debian 13 container, `m8-debian` | curl; systemd user unit active; Cargo absent | `M8_m8_OK`, session `01M2EGM1FZNVTAMSM5R0WFYX3D` |
| iMac, `Nicholas-Work.local` | curl over SSH; launchd agent running | `M8_nicholascaron_OK`, session `01M2EGMC6D297HXPE80T3F0Y1G` |
| Windows PC, `nicholas` | PowerShell over SSH; Den Host Scheduled Task running | `M8_caron_OK`, session `01M2EGQDCMGQRZP82NTWMQH23V` |

All three terminal screenshots were inspected: [Debian](shots/m8-host-debian.png),
[macOS](shots/m8-host-macos.png), [Windows](shots/m8-host-windows.png).
The iMac shell reports missing optional zoxide/atuin prompt helpers under launchd's
PATH, but executes commands and renders output correctly. Its existing shell
configuration was not changed. Native installers leave the iMac and PC enrolled.
The smoke closes its test terminals, leaving their labeled cards/recordings.
The disposable Debian enrollment and container were removed afterward.

`scripts/m8-host-smoke.mjs` repeats the real browser check for `DEN_SMOKE_HOST`.
`scripts/m8-installer-check.sh` proves a corrupted binary fails before installation
or enrollment. Local workspace tests passed, including 19 server behaviors and
two real host tests; Clippy with warnings denied, formatting, and web checks passed.
The built Machines UI passed keyboard checks and visual inspection on
[desktop](shots/m8-machines-desktop.png) and [mobile](shots/m8-machines-mobile.png).
Screenshots replace the one-time enrollment code with a placeholder.

Public CI exposed old Clippy warnings and an M1 smoke assumption that the first
room accepts text. Those fixes are committed separately. The corrected M1 smoke
passed real CLI chat/reconnect, an 11,894,863-byte 41-second clip with authenticated
ranges and complete HTTP decode, and bot token revocation/replacement.
[Public CI run 34788906199](https://github.com/nhclink16/den/actions/runs/34788906199)
passed every step, including SQLx migration metadata verification.

## 2. Offline export and import

`den-server export backup.zip` reads `DEN_DB` and `DEN_UPLOADS`. Stop the server
first: the CLI refuses an active system or user unit and takes an exclusive SQLite
lock, including against WAL readers. It keeps the lock while copying a VACUUM
snapshot and the flat upload directory. Partial files, offsets and recordings are
included; deployment configuration, `.ssh` and host configuration files are not.
Archives contain private chat and password/token hashes; keep them private.

`den-server import backup.zip --into /path/to/empty-data-dir` validates every file
and the migration history before applying normal migrations in a private sibling
directory. Installation is one rename after integrity and foreign-key checks.
Existing data is never overwritten. Limits are 100,000 entries, 1 TiB expanded,
and a 32 MiB manifest, with a free-space check before extraction. Traversal,
symlinks, duplicate entries, mismatched hashes, newer schemas and divergent
migrations are rejected. Export writes uncompressed ZIP entries so already
compressed media does not lengthen the offline window.

Default import deletes sessions, API tokens, invites and enrollments, invalidates
host credentials, revokes grants and finalizes active terminal recordings. Password
login still works; hosts must enroll again. `--keep-credentials` preserves access
for disaster recovery, but terminals still end and hosts start offline. Deploy
LiveKit and other external configuration separately; none is copied from an
archive. The import summary reports invalidation counts without credentials.

Two integration tests run the actual subcommands and boot the imported server.
They verify history, authenticated range requests, upload resume, recording access,
credential invalidation/preservation, refusal cases, and migration from schema 4.
Both pass. Import waits for SQLite's asynchronous pool shutdown; export still
uses a zero-timeout exclusive lock to reject a running database immediately.

The nightly unit stops Den for export, restarts it before the network transfer,
and also restarts it on failure. `den.zip` goes to the existing dated codexbox
backup destination, with the completion marker last and retention unchanged.
The restore drill now imports that archive before booting the restored server.
