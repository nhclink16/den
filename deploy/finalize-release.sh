#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
tag=${1:?Usage: finalize-release.sh <tag> [output-dir]}
out=${2:-release-artifacts}
[[ $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$ ]] || exit 1
git archive --format=tar.gz --prefix="den-$tag/" "$tag" > "$out/den-source-$tag.tar.gz"
cp LICENSE "$out/LICENSE"
cp LICENSES/*.txt "$out/"
python3 - "$out" <<'PY'
import hashlib, pathlib, sys
root = pathlib.Path(sys.argv[1])
with (root / 'SHA256SUMS').open('w') as sums:
    for p in sorted(root.iterdir()):
        if p.is_file() and p.name != 'SHA256SUMS':
            with p.open('rb') as f:
                digest = hashlib.file_digest(f, 'sha256').hexdigest()
            sums.write(f'{digest}  {p.name}\n')
PY
