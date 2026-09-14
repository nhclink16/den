#!/usr/bin/env python3
"""Portable, private, loopback-only Den fixture for an individual Cloud test machine."""
import argparse
import json
import os
from pathlib import Path
import secrets
import shutil
import signal
import socket
import subprocess
import tempfile
import time
import urllib.request


OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def request(origin, path, body=None, token=None):
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = "Bearer " + token
    request = urllib.request.Request(origin + path, headers=headers,
        data=json.dumps(body).encode() if body is not None else None)
    with OPENER.open(request, timeout=10) as response:
        return json.load(response)


def private_json(path, value):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as output:
        json.dump(value, output)
        output.write("\n")


def receipt_path(credentials):
    return credentials.with_name(credentials.name + ".receipt.json")


def fingerprint(pid):
    result = subprocess.run(["/bin/ps", "-p", str(pid), "-o", "lstart=", "-o", "args="],
                            text=True, capture_output=True)
    return result.stdout.strip() if result.returncode == 0 else None


def stop(credentials):
    receipt_file = receipt_path(credentials)
    if not receipt_file.exists():
        if credentials.exists():
            raise RuntimeError("Credentials exist without an ownership receipt; refusing cleanup")
        return
    receipt = json.loads(receipt_file.read_text())
    directory = Path(receipt["directory"]).resolve()
    if directory.parent != Path(tempfile.gettempdir()).resolve() or not directory.name.startswith("den-cloud-fixture-"):
        raise RuntimeError("Receipt points outside the exact temporary fixture directory")
    if receipt["credentials"] != str(credentials):
        raise RuntimeError("Fixture credentials do not match the receipt")
    current = fingerprint(receipt["pid"])
    if current is not None:
        if current != receipt["fingerprint"]:
            raise RuntimeError("PID was reused; refusing to signal another process")
        os.kill(receipt["pid"], signal.SIGINT)
        for _ in range(100):
            if fingerprint(receipt["pid"]) is None:
                break
            time.sleep(0.1)
        else:
            raise RuntimeError("Fixture did not exit; files preserved")
    credentials.unlink(missing_ok=True)
    receipt_file.unlink()
    shutil.rmtree(directory)
    print("Stopped the exact Cloud fixture and removed its private files")


def start(binary, credentials):
    if credentials.exists() or receipt_path(credentials).exists():
        raise RuntimeError("Fixture output already exists; stop its owner before starting another")
    directory = Path(tempfile.mkdtemp(prefix="den-cloud-fixture-")).resolve()
    process = None
    try:
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        origin = f"http://127.0.0.1:{port}"
        # A private executable path and start time make PID cleanup specific to this process.
        executable = directory / "den-server"
        executable.symlink_to(binary)
        env = {key: value for key, value in os.environ.items() if not key.startswith("DEN_")}
        env.update(DEN_BIND=f"127.0.0.1:{port}", DEN_ORIGIN=origin,
                   DEN_DB=str(directory / "den.db"), DEN_UPLOADS=str(directory / "uploads"),
                   DEN_BOOTSTRAP_FILE=str(directory / "bootstrap.key"))
        with (directory / "server.log").open("w") as log:
            process = subprocess.Popen([str(executable)], cwd=directory, env=env,
                stdout=log, stderr=log, start_new_session=True)
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError("Fixture server exited; its log is private")
            try:
                request(origin, "/health")
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise RuntimeError("Fixture server did not become ready")
        first_password, second_password = secrets.token_urlsafe(24), secrets.token_urlsafe(24)
        first = request(origin, "/auth/init", {
            "username": "ios_alex", "password": first_password,
            "bootstrap_token": (directory / "bootstrap.key").read_text(),
        })
        invite = request(origin, "/invites", {"uses": 1, "expires_in_hours": 24}, first["token"])
        second = request(origin, "/auth/register", {
            "username": "ios_blair", "password": second_password, "invite": invite["code"],
        })
        dm = request(origin, "/dms", {"member_ids": [second["user"]["id"]]}, first["token"])
        channels = request(origin, "/channels", token=first["token"])
        general = next(channel for channel in channels if channel["kind"] == "text")
        request(origin, f'/channels/{general["id"]}/messages',
            {"content": "Cloud fixture ready. These are disposable test conversations."}, second["token"])
        private_json(receipt_path(credentials), {
            "pid": process.pid, "fingerprint": fingerprint(process.pid), "directory": str(directory),
            "credentials": str(credentials),
        })
        private_json(credentials, {
            "origin": origin, "port": port, "general_channel_id": general["id"],
            "dm_channel_id": dm["id"], "livekit_configured": False,
            "users": [{"username": "ios_alex", "password": first_password, "session": first},
                      {"username": "ios_blair", "password": second_password, "session": second}],
        })
        print(f"Isolated Cloud fixture ready on loopback port {port}; credentials stored privately")
    except BaseException:
        if process and process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
        credentials.unlink(missing_ok=True)
        receipt_path(credentials).unlink(missing_ok=True)
        shutil.rmtree(directory)
        raise


if __name__ == "__main__":
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["start", "stop"])
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--credentials", type=Path, default=Path("/tmp/den-ios-cloud-fixture.json"))
    args = parser.parse_args()
    credentials = args.credentials.absolute()
    if args.command == "start":
        if not args.binary or not args.binary.is_file():
            parser.error("start requires --binary pointing to a built den-server")
        start(args.binary.resolve(), credentials)
    else:
        stop(credentials)
