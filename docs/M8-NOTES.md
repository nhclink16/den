# M8 portable

Work in progress. Item 1 is underway; export/import and instance name have not started.

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

The agreed split is now explicit in package metadata and LICENSE: AGPL-3.0-only
for den-server and den-host, MIT for original client, CLI, shared types and plugin
code. Third-party notices, including tldraw's separate license, remain in place.
Earlier MIT versions retain their permissions.

Before the visibility change, GitHub check 103792472606 could not start because
of account payments or the spending limit. Both native acceptance machines were
reachable: Nicholas-Work.local as nicholascaron, Apple arm64, and nicholas as the
Windows x86_64 PC. The VPS identified itself as vps-2fd9743a with Den active.
