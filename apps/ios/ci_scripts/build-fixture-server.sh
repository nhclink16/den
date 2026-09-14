#!/bin/sh
set -eu
cd "$(dirname "$0")"
den_work=$(mktemp -d "${TMPDIR:-/tmp}/den-cloud-rust.XXXXXX")
den_rust_version='1.98.1'
# The test machine has ci_scripts, not the repository checkout or the build machine's /tmp.
python3 - "$den_work" <<'PY'
import pathlib, sys, tarfile
root = pathlib.Path(sys.argv[1]) / 'source'
root.mkdir()
with tarfile.open('fixture-source.tar.gz', 'r:gz') as archive:
    for entry in archive.getmembers():
        path = pathlib.PurePosixPath(entry.name)
        if not entry.isfile() or path.is_absolute() or '..' in path.parts:
            raise SystemExit('Invalid entry in the committed Rust fixture source archive')
    for entry in archive.getmembers():
        destination = root / entry.name
        destination.parent.mkdir(parents=True, exist_ok=True)
        with archive.extractfile(entry) as source:
            destination.write_bytes(source.read())
PY
if command -v rustc >/dev/null 2>&1 && [ "$(rustc --version | cut -d ' ' -f 2)" = "$den_rust_version" ]; then
    den_cargo=$(command -v cargo)
else
    case "$(uname -m)" in
        arm64) den_host='aarch64-apple-darwin'; den_digest='20ef5516c31b1ac2290084199ba77dbbcaa1406c45c1d978ca68558ef5964ef5' ;;
        x86_64) den_host='x86_64-apple-darwin'; den_digest='9c331076f62b4d0edeae63d9d1c9442d5fe39b37b05025ec8d41c5ed35486496' ;;
        *) echo 'Unsupported Cloud host architecture' >&2; exit 1 ;;
    esac
    curl --fail --location --silent --show-error --retry 2 \
      "https://static.rust-lang.org/rustup/archive/1.28.2/$den_host/rustup-init" --output "$den_work/rustup-init"
    printf '%s  %s\n' "$den_digest" "$den_work/rustup-init" | shasum -a 256 --check >&2
    chmod +x "$den_work/rustup-init"
    export RUSTUP_HOME="$den_work/rustup" CARGO_HOME="$den_work/cargo"
    "$den_work/rustup-init" -y --no-modify-path --profile minimal --default-toolchain "$den_rust_version" >&2
    den_cargo="$CARGO_HOME/bin/cargo"
fi
cd "$den_work/source"
"$den_cargo" build --locked --manifest-path "$den_work/source/Cargo.toml" \
  --target-dir "$den_work/target" -p den-server >&2
printf '%s\n' "$den_work/target/debug/den-server"
