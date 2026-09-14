#!/usr/bin/env python3
"""Start/stop an isolated native UI test server. Never print fixture credentials."""
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


def request(origin, path, body=None, token=None):
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = "Bearer " + token
    req = urllib.request.Request(
        origin + path, headers=headers,
        data=json.dumps(body).encode() if body is not None else None,
    )
    with urllib.request.urlopen(req, timeout=10) as response:
        return json.load(response)


def private_json(path, value):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as out:
        json.dump(value, out, indent=2)
        out.write("\n")


def free_port(kind=socket.SOCK_STREAM, address="127.0.0.1"):
    with socket.socket(type=kind) as listener:
        listener.bind((address, 0))
        return listener.getsockname()[1]


def start_livekit(directory, origin):
    media_ip = subprocess.check_output(["tailscale", "ip", "-4"], text=True).strip()
    # Validate before using an address in Docker/network configuration.
    import ipaddress
    if ipaddress.ip_address(media_ip) not in ipaddress.ip_network("100.64.0.0/10"):
        raise RuntimeError("Expected this machine's Tailnet IPv4 address")
    port = free_port()
    udp = free_port(socket.SOCK_DGRAM, media_ip)
    key, secret = "fixture" + secrets.token_hex(8), secrets.token_urlsafe(32)
    name = directory.name + "-livekit"
    config = directory / "livekit.json"
    private_json(config, {
        "port": port, "bind_addresses": ["127.0.0.1"],
        "rtc": {"node_ip": media_ip, "udp_port": udp, "tcp_port": 0,
                "use_external_ip": False, "ips": {"includes": [media_ip + "/32"]}},
        "turn": {"enabled": False}, "keys": {key: secret},
        "webhook": {"api_key": key, "urls": [origin + "/livekit/webhook"]},
        "logging": {"level": "warn"},
        "room": {"departure_timeout": 5},
    })
    container = subprocess.check_output([
        "docker", "run", "-d", "--name", name, "--network", "host",
        "--label", "den.fixture=" + str(directory),
        "--mount", f"type=bind,src={config},dst=/fixture.json,readonly",
        "livekit/livekit-server:v1.9.0", "--config", "/fixture.json",
    ], text=True).strip()
    # Record exact ownership immediately so failure cleanup has a durable receipt.
    private_json(directory / "livekit-receipt.json", {"container": container, "directory": str(directory)})
    for _ in range(100):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}", timeout=1):
                return {"DEN_LIVEKIT_URL": f"ws://127.0.0.1:{port}",
                        "DEN_LIVEKIT_API_KEY": key, "DEN_LIVEKIT_API_SECRET": secret}, {
                            "livekit_configured": True, "livekit_port": port,
                            "livekit_media_ip": media_ip, "livekit_udp_port": udp}
        except OSError:
            time.sleep(0.1)
    raise RuntimeError("Isolated LiveKit failed to start; inspect its container log privately")


def stop_livekit(directory):
    path = directory / "livekit-receipt.json"
    if not path.exists():
        return
    receipt = json.loads(path.read_text())
    inspect = subprocess.run(["docker", "inspect", receipt["container"]], capture_output=True, text=True)
    if inspect.returncode == 0:
        instance = json.loads(inspect.stdout)[0]
        if instance["Config"]["Labels"].get("den.fixture") != str(directory):
            raise RuntimeError("Container ownership mismatch; refusing cleanup")
        subprocess.run(["docker", "rm", "-f", receipt["container"]], check=True, stdout=subprocess.DEVNULL)


def start(binary, livekit=False):
    directory = Path(tempfile.mkdtemp(prefix="den-ios-fixture-", dir="/mnt/storage"))
    process = None
    try:
        port = free_port()
        origin = f"http://127.0.0.1:{port}"
        env = {key: value for key, value in os.environ.items() if not key.startswith("DEN_")}
        env.update(DEN_BIND=f"127.0.0.1:{port}", DEN_ORIGIN=origin,
                   DEN_DB=str(directory / "den.db"), DEN_UPLOADS=str(directory / "uploads"),
                   DEN_BOOTSTRAP_FILE=str(directory / "bootstrap.key"))
        media = {"livekit_configured": False}
        if livekit:
            media_env, media = start_livekit(directory, origin)
            env.update(media_env)
        with (directory / "server.log").open("w") as log:
            process = subprocess.Popen([str(binary)], cwd=directory, env=env,
                                       stdout=log, stderr=log, start_new_session=True)
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError("Fixture server exited; inspect its private log")
            try:
                request(origin, "/health")
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise RuntimeError("Fixture server failed to become ready")
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
                {"content": "Native fixture ready. These are disposable test conversations."}, second["token"])
        credentials = directory / "credentials.json"
        private_json(credentials, {
            "origin": origin, "port": port, "general_channel_id": general["id"],
            "dm_channel_id": dm["id"], **media,
            "users": [{"username": "ios_alex", "password": first_password, "session": first},
                      {"username": "ios_blair", "password": second_password, "session": second}],
        })
        private_json(directory / "receipt.json", {
            "pid": process.pid, "binary": str(binary.resolve()), "directory": str(directory),
            "port": port, "process_start": Path(f"/proc/{process.pid}/stat").read_text().split()[21],
        })
        print(json.dumps({"port": port, "credentials_file": str(credentials),
                          **({"livekit_port": media["livekit_port"]} if livekit else {})}))
    except BaseException:
        if process and process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
        stop_livekit(directory)
        if os.environ.get("DEN_SMOKE_KEEP") != "1":
            shutil.rmtree(directory)
        raise


def stop(directory):
    directory = directory.resolve()
    if directory.parent != Path("/mnt/storage") or not directory.name.startswith("den-ios-fixture-"):
        raise RuntimeError("Expected an exact fixture directory under /mnt/storage")
    receipt = json.loads((directory / "receipt.json").read_text())
    if receipt["directory"] != str(directory):
        raise RuntimeError("Fixture receipt does not match directory")
    pid = receipt["pid"]
    proc = Path(f"/proc/{pid}")
    if proc.exists():
        fields = (proc / "stat").read_text().split()
        env = (proc / "environ").read_bytes().split(b"\0")
        if fields[21] != receipt["process_start"] or f"DEN_DB={directory}/den.db".encode() not in env:
            raise RuntimeError("PID no longer identifies this fixture server; refusing to stop it")
        os.kill(pid, signal.SIGINT)
        for _ in range(100):
            if not proc.exists() or (proc / "stat").read_text().split()[2] == "Z":
                break
            time.sleep(0.1)
        else:
            raise RuntimeError("Fixture did not stop; its files were preserved")
    stop_livekit(directory)
    if os.environ.get("DEN_SMOKE_KEEP") == "1":
        print("Stopped fixture; DEN_SMOKE_KEEP=1 preserved its directory")
    else:
        shutil.rmtree(directory)
        print("Stopped fixture and removed its exact isolated directory")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("start")
    create.add_argument("--binary", type=Path, default=Path(os.environ.get(
        "CARGO_TARGET_DIR", "/mnt/storage/den-m5-target")) / "debug/den-server")
    create.add_argument("--livekit", action="store_true", help="Private two-peer media on this Tailnet node")
    remove = commands.add_parser("stop")
    remove.add_argument("--dir", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "start":
        start(args.binary.resolve(), args.livekit)
    else:
        stop(args.dir)
