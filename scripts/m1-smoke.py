#!/usr/bin/env python3
"""Exercise M1 through the built CLI and a disposable server. Requires ffmpeg."""
import json
from contextlib import nullcontext
import os
from pathlib import Path
import queue
import socket
import subprocess
import tempfile
import threading
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
BUILD = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug"
DEN = BUILD / "den"
SERVER = BUILD / "den-server"


def main():
    keep = os.environ.get("DEN_SMOKE_KEEP") == "1"
    scratch = nullcontext(tempfile.mkdtemp(prefix="den-m1-", dir="/mnt/storage")) if keep else tempfile.TemporaryDirectory(prefix="den-m1-", dir="/mnt/storage")
    with scratch as tmp:
        tmp = Path(tmp)
        with socket.socket() as probe:
            probe.bind(("127.0.0.1", 0))
            port = probe.getsockname()[1]
        url = f"http://127.0.0.1:{port}"
        env = {k: v for k, v in os.environ.items() if not k.startswith("DEN_")}
        env.update(DEN_URL=url, DEN_BIND=f"127.0.0.1:{port}", DEN_ORIGIN=url,
                   DEN_DB=str(tmp / "den.db"), DEN_UPLOADS=str(tmp / "uploads"),
                   DEN_BOOTSTRAP_FILE=str(tmp / "bootstrap.key"))
        processes = []
        with (tmp / "server.log").open("w") as log:
            server = subprocess.Popen([str(SERVER)], env=env, stdout=log, stderr=log)
            processes.append(server)
            try:
                for _ in range(100):
                    try:
                        urllib.request.urlopen(url + "/health", timeout=1).close()
                        break
                    except OSError:
                        assert server.poll() is None, "Server exited during startup"
                        time.sleep(0.05)
                else:
                    raise AssertionError("Server did not start")

                def cli(account, *args, token=None):
                    call_env = dict(env, DEN_CONFIG=str(tmp / f"{account}.json"),
                                    DEN_PASSWORD="smoke-password-123")
                    if token:
                        call_env["DEN_TOKEN"] = token
                    result = subprocess.run([str(DEN), *args], env=call_env,
                                            capture_output=True, text=True, timeout=60)
                    assert result.returncode == 0, f"den {args[0]} failed: {result.stderr}"
                    return json.loads(result.stdout) if result.stdout.strip() else None

                admin = cli("alice", "init", "alice", "--bootstrap-file", str(tmp / "bootstrap.key"))
                invite = cli("alice", "invite", "create")
                bob = cli("bob", "register", "bob", "--invite", invite["code"])
                cli("bob", "login", "bob")
                general = next(c["id"] for c in cli("alice", "channels") if c["kind"] == "text" and c["name"] == "general")

                def tail(account):
                    proc = subprocess.Popen([str(DEN), "tail", general],
                                            env=dict(env, DEN_CONFIG=str(tmp / f"{account}.json")),
                                            stdout=subprocess.PIPE, stderr=log, text=True, bufsize=1)
                    processes.append(proc)
                    messages = queue.Queue()
                    def read():
                        for line in proc.stdout:
                            event = json.loads(line)
                            if event["type"] in ("resync", "message_created"):
                                messages.put(event)
                    threading.Thread(target=read, daemon=True).start()
                    assert messages.get(timeout=10)["type"] == "resync"
                    return messages

                alice_events, bob_events = tail("alice"), tail("bob")
                first = cli("alice", "send", general, "hello from Alice's terminal")
                second = cli("bob", "send", general, "hello from Bob's terminal")
                for events in (alice_events, bob_events):
                    assert [events.get(timeout=10)["id"] for _ in range(2)] == [first["id"], second["id"]]
                history = cli("alice", "read", general)
                assert [m["author_id"] for m in history] == [admin["id"], bob["id"]]
                print("PASS: two independent CLI sessions send, read and tail both messages", flush=True)

                server.terminate()
                server.wait(timeout=5)
                server = subprocess.Popen([str(SERVER)], env=env, stdout=log, stderr=log)
                processes.append(server)
                for events in (alice_events, bob_events):
                    assert events.get(timeout=15)["type"] == "resync"
                assert len(cli("bob", "read", general)) == 2
                print("PASS: both den tail processes reconnect after server restart; history and sessions persist", flush=True)

                # Enough bitrate to cross the CLI's 4 MiB chunk boundary.
                clip = tmp / "clip.mp4"
                subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y",
                                "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=24",
                                "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000",
                                "-t", "41", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "18",
                                "-pix_fmt", "yuv420p", "-c:a", "aac", "-movflags", "+faststart", str(clip)],
                               check=True, timeout=90)
                assert clip.stat().st_size > 4 * 1024 * 1024
                upload = cli("alice", "upload", general, str(clip))
                assert upload["complete"] and upload["offset"] == clip.stat().st_size
                attached = cli("alice", "send", general, "41-second clip", "--upload", upload["id"])
                assert attached["attachments"][0]["id"] == upload["id"]
                session = json.loads((tmp / "alice.json").read_text())[url]["token"]
                media_url = f"{url}/uploads/{upload['id']}/file"
                request = urllib.request.Request(media_url, headers={"Authorization": f"Bearer {session}", "Range": "bytes=4096-8191"})
                with urllib.request.urlopen(request) as response:
                    assert response.status == 206
                    assert response.headers["Content-Range"] == f"bytes 4096-8191/{clip.stat().st_size}"
                    with clip.open("rb") as source:
                        source.seek(4096)
                        assert response.read() == source.read(4096)
                request = urllib.request.Request(media_url, headers={"Authorization": f"Bearer {session}"}, method="HEAD")
                with urllib.request.urlopen(request) as response:
                    assert int(response.headers["Content-Length"]) == clip.stat().st_size
                playback = subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-headers",
                                           f"Authorization: Bearer {session}\r\n", "-i", media_url,
                                           "-f", "null", "-"], capture_output=True, timeout=60)
                assert playback.returncode == 0, "HTTP clip decoding failed"
                print(f"PASS: 41-second H.264/AAC clip ({clip.stat().st_size} bytes), chunk upload, authenticated HEAD/range, and full HTTP decode", flush=True)

                bot = cli("alice", "bot", "create", "clanker")
                message = cli("agent", "send", general, "agent joined with a bearer token", token=bot["credential"]["token"])
                assert message["author_id"] == bot["user"]["id"]
                assert bot["user"]["bot"] and bot["user"]["role"] == "member"
                cli("alice", "token", "revoke", bot["credential"]["credential"]["id"])
                replacement = cli("alice", "token", "create", "replacement", "--user", bot["user"]["id"])
                assert cli("agent", "me", token=replacement["token"])["id"] == bot["user"]["id"]
                print("PASS: agent posts as a bot; owner revokes and replaces its token", flush=True)
            finally:
                for proc in reversed(processes):
                    proc.terminate()
                for proc in processes:
                    try:
                        proc.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        proc.kill()
                        proc.wait()

    if keep:
        print(f"DEN_SMOKE_KEEP=1: retained disposable server data at {tmp}")
    else:
        assert not tmp.exists(), "Disposable smoke database and uploads must be removed"
        print("Cleanup verified: disposable database, messages, objects, requests and uploads removed.")


if __name__ == "__main__":
    main()
