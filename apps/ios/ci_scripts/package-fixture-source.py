#!/usr/bin/env python3
"""Create/check the deterministic source bundle carried to Xcode Cloud test machines."""
import argparse
import gzip
import hashlib
import io
from pathlib import Path
import subprocess
import tarfile


def archive(repository):
    tracked = subprocess.check_output([
        "git", "-C", str(repository), "ls-files", "-z", "--",
        "Cargo.toml", "Cargo.lock", ".cargo", ".sqlx", "crates", "LICENSE", "LICENSES",
    ]).decode().split("\0")
    files = sorted(path for path in tracked if path)
    if not {"Cargo.toml", "Cargo.lock", "crates/den-server/Cargo.toml"}.issubset(files):
        raise RuntimeError("The tracked Rust workspace is incomplete")
    buffer = io.BytesIO()
    with gzip.GzipFile(fileobj=buffer, mode="wb", filename="", mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as output:
            for name in files:
                source = repository / name
                if source.is_symlink() or not source.is_file():
                    raise RuntimeError(f"Fixture source must be a regular tracked file: {name}")
                contents = source.read_bytes()
                item = tarfile.TarInfo(name)
                item.size = len(contents)
                item.mode = 0o644
                item.mtime = item.uid = item.gid = 0
                item.uname = item.gname = ""
                output.addfile(item, io.BytesIO(contents))
    return buffer.getvalue(), len(files)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    destination = Path(__file__).with_name("fixture-source.tar.gz")
    contents, count = archive(root)
    if args.check:
        if not destination.exists() or destination.read_bytes() != contents:
            raise SystemExit("Rust fixture source bundle is stale. Run apps/ios/ci_scripts/package-fixture-source.py and commit the archive.")
    else:
        destination.write_bytes(contents)
    print(f"Fixture source: {count} tracked files, {len(contents)} bytes, sha256 {hashlib.sha256(contents).hexdigest()}")
