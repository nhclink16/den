#!/usr/bin/env bash
set -euo pipefail
umask 077
exec 9>/var/lib/den/.backup.lock
flock -n 9 || exit 0
archive=/var/lib/den/.backup.zip
if [[ ${1:-} = snapshot ]]; then
  # The systemd unit stops Den first and restarts it even when export fails.
  rm -f "$archive.next"
  /opt/den/bin/den-server export "$archive.next"
  mv -f "$archive.next" "$archive"
  exit 0
fi
[[ ${1:-} = transfer ]] || { echo 'Run systemctl start den-backup.service' >&2; exit 1; }
snapshot=$(mktemp -d /var/lib/den/.backup.XXXXXX)
trap 'rm -rf "$snapshot"' EXIT
date=$(date -u +%F)
transport='ssh -i /var/lib/den/.ssh/backup_ed25519 -o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=10'
printf '%s\n' incomplete > "$snapshot/complete"
rsync -a -e "$transport" "$snapshot/complete" den-backup@100.116.27.23:"$date/"
rsync -a -e "$transport" "$archive" den-backup@100.116.27.23:"$date/den.zip"
# Sent last: an interrupted transfer never becomes a completed snapshot.
printf '%s\n' "$(date -u +%FT%TZ)" > "$snapshot/complete"
rsync -a -e "$transport" "$snapshot/complete" den-backup@100.116.27.23:"$date/"
rm -f "$archive"
echo "Den backup complete: $date"
