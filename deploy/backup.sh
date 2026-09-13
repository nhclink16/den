#!/usr/bin/env bash
set -euo pipefail
umask 077
exec 9>/var/lib/den/.backup.lock
flock -n 9 || exit 0
snapshot=$(mktemp -d /var/lib/den/.backup.XXXXXX)
trap 'rm -rf "$snapshot"' EXIT
sqlite3 /var/lib/den/den.db ".timeout 10000" "VACUUM INTO '$snapshot/den.db';"
[[ $(sqlite3 "$snapshot/den.db" 'PRAGMA integrity_check;') = ok ]]
date=$(date -u +%F)
transport='ssh -i /var/lib/den/.ssh/backup_ed25519 -o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=10'
printf '%s\n' incomplete > "$snapshot/complete"
rsync -a -e "$transport" "$snapshot/complete" den-backup@100.116.27.23:"$date/"
rsync -a -e "$transport" "$snapshot/den.db" den-backup@100.116.27.23:"$date/"
rsync -a -e "$transport" /var/lib/den/uploads/ den-backup@100.116.27.23:"$date/uploads/"
# Sent last: an interrupted transfer never becomes a completed snapshot.
printf '%s\n' "$(date -u +%FT%TZ)" > "$snapshot/complete"
rsync -a -e "$transport" "$snapshot/complete" den-backup@100.116.27.23:"$date/"
echo "Den backup complete: $date"
