#!/usr/bin/env python3
"""Ready one explicit simulator, reset only Den, and issue a single-use test receipt."""
import argparse
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile
import time
import urllib.parse
import uuid


BUNDLE_ID = "app.denchat.ios"


def reset(simulator, fixture):
    simulator = str(uuid.UUID(simulator)).upper()  # Reject aliases such as "booted".
    fixture = Path(fixture).resolve(strict=True)
    if not fixture.is_file():
        raise ValueError("The isolated fixture must be a regular file")
    configuration = json.loads(fixture.read_text())
    origin = urllib.parse.urlsplit(configuration["origin"])
    if origin.scheme != "http" or origin.hostname not in {"127.0.0.1", "localhost", "::1"}:
        raise ValueError("Privacy reset requires an isolated loopback fixture")
    if not isinstance(configuration.get("users"), list) or len(configuration["users"]) < 2:
        raise ValueError("The isolated fixture must contain disposable test accounts")
    receipt = Path(str(fixture) + ".dictation-reset.json")
    if receipt.is_symlink() or (receipt.exists() and not stat.S_ISREG(receipt.stat().st_mode)):
        raise ValueError("The reset receipt path is not a regular file")
    # Cloud's pre-test hook may run before its selected destination is booted.
    # This is a no-op for a ready device and never chooses a different simulator.
    subprocess.run(["/usr/bin/xcrun", "simctl", "bootstatus", simulator, "-b"], check=True, timeout=180)
    command = ["/usr/bin/xcrun", "simctl", "privacy", simulator, "reset", "all", BUNDLE_ID]
    # Do not grant anything, reset other apps, or launch capture here.
    subprocess.run(command, check=True, timeout=30)
    value = {"nonce": str(uuid.uuid4()).upper(), "bundleID": BUNDLE_ID,
             "simulatorID": simulator, "fixturePath": str(fixture),
             "resetAt": time.time(), "resetExitCode": 0, "command": command}
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", dir=receipt.parent, prefix=".dictation-reset-", delete=False) as output:
            temporary = Path(output.name)
            os.chmod(temporary, 0o600)
            json.dump(value, output, indent=2)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, receipt)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    print(f"Reset Den privacy on exact simulator {simulator}; fresh single-use receipt: {receipt}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--simulator", required=True, help="Exact simulator UUID; aliases are rejected")
    parser.add_argument("--fixture", required=True, type=Path, help="Private loopback fixture JSON")
    arguments = parser.parse_args()
    reset(arguments.simulator, arguments.fixture)
