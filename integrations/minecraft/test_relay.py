"""Exercise the relay with temporary executables and a local fake Den only."""
import asyncio
import base64
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import shlex
import sys
import tempfile
import threading
import unittest
import urllib.error
import urllib.request

from websockets.asyncio.server import serve

RELAY = Path(__file__).with_name("den-minecraft-relay")
ICON = base64.b64decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=")
FAKE = '''#!{python}
import json, os, pathlib, sys, time
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
record = {{"executable": name, "args": args}}
if name == "mcrcon":
    record["rcon_env_ok"] = (os.environ.get("MCRCON_HOST") == "fake-host" and
        os.environ.get("MCRCON_PORT") == "12345" and os.environ.get("MCRCON_PASS") == "fake-rcon-secret")
with open(os.environ["FAKE_LOG"], "a") as log:
    log.write(json.dumps(record) + "\\n")
state = json.loads(pathlib.Path(os.environ["FAKE_STATE"]).read_text())
if name == "systemctl":
    if args == ["is-active", "fake-unit"]:
        print(state.get("active", "active"))
        sys.exit(0 if state.get("active", "active") == "active" else 3)
    elif args == ["show", "-p", "MainPID", "--value", "fake-unit"]:
        print(os.getppid() if state.get("active", "active") != "inactive" else 0)
    else:
        sys.exit(4)
elif name == "mcrcon":
    if args == ["-c", "list"]:
        if state.get("list_fail"):
            print("RCON unavailable", file=sys.stderr)
            sys.exit(1)
        print("There are 2 of a max of 5 players online: Steve_1984, Alex")
    elif args == ["-c", "spark tps"]:
        print("§7TPS from last 5s, 10s, 1m, 5m, 15m:\\n§a*20.0, §e19.9, 20.0, 20.0, 20.0")
    elif args[1].startswith("say ") and state.get("say_fail"):
        print("announcement failed", file=sys.stderr)
        sys.exit(1)
elif name in ("start", "stop", "restart"):
    time.sleep(0.6 if name == "restart" else 0)
    if state.get("command_fail") == name:
        print("failure:" + "x" * 250, file=sys.stderr)
        sys.exit(1)
else:
    sys.exit(5)
'''


class RelayTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.mc = self.root / "server"
        self.mc.mkdir()
        (self.mc / "server.properties").write_text("motd=  §aFriend §lworld§r  \nlevel-name=custom-world\n")
        (self.mc / "start.sh").write_text("java -jar fabric-server-mc.1.21.1-loader.0.16.0-launcher.1.0.1.jar nogui\n")
        (self.mc / "mods").mkdir()
        for name in ("spark.jar", "other.jar", "ignored.txt"):
            (self.mc / "mods" / name).write_text("")
        (self.mc / "server-icon.png").write_bytes(ICON)
        (self.mc / "usercache.json").write_text(json.dumps([
            {"name": "Steve_1984", "uuid": "steve-uuid"}, {"name": "Alex", "uuid": "alex-uuid"},
            {"name": "MissingStats", "uuid": "missing-uuid"},
        ]))
        stats = self.mc / "custom-world/stats"
        stats.mkdir(parents=True)
        for uuid, ticks in (("steve-uuid", 2400), ("alex-uuid", 601)):
            (stats / f"{uuid}.json").write_text(json.dumps({"stats": {"minecraft:custom": {"minecraft:play_time": ticks}}}))
        self.mapping = self.root / "players.json"
        self.mapping.write_text('{"steve_1984": "NERc"}')
        self.backups = self.root / "backups"
        self.backups.mkdir()
        for name, mtime in (("world-old.zip", 1600000000), ("world-new.zip", 1700000000), ("other", 1800000000)):
            path = self.backups / name
            path.write_text("")
            os.utime(path, (mtime, mtime))
        self.log = self.root / "commands.log"
        self.state = self.root / "state.json"
        self.state.write_text("{}")
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for name in ("mcrcon", "systemctl", "start", "stop", "restart"):
            executable = self.bin / name
            executable.write_text(FAKE.format(python=sys.executable))
            executable.chmod(0o755)
        self.http_requests = []
        self.users_fail = False
        test = self

        class Users(BaseHTTPRequestHandler):
            def do_GET(self):
                test.http_requests.append((self.path, self.headers.get("Authorization")))
                body = json.dumps([{"username": "nerc", "id": "01DENUSER"}]).encode()
                self.send_response(503 if test.users_fail else 200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *args):
                pass

        self.http = ThreadingHTTPServer(("127.0.0.1", 0), Users)
        self.http_thread = threading.Thread(target=self.http.serve_forever, daemon=True)
        self.http_thread.start()
        self.frames = asyncio.Queue()
        self.connections = asyncio.Queue()
        self.connection_count = 0

        async def process_request(connection, request):
            if request.path == "/servers/relay":
                return None
            # Keep DEN_URL one origin; serve /users through the tiny HTTP server.
            def fetch():
                req = urllib.request.Request(f"http://127.0.0.1:{self.http.server_port}{request.path}",
                                             headers={"Authorization": request.headers.get("Authorization", "")})
                try:
                    with urllib.request.urlopen(req, timeout=2) as response:
                        return HTTPStatus.OK, response.read().decode()
                except urllib.error.HTTPError as error:
                    return HTTPStatus(error.code), error.read().decode()
            status, body = await asyncio.to_thread(fetch)
            return connection.respond(status, body)

        async def handler(ws):
            self.connection_count += 1
            await self.connections.put(ws)
            async for raw in ws:
                await self.frames.put(json.loads(raw))

        self.ws_server = await serve(handler, "127.0.0.1", 0, process_request=process_request)
        port = self.ws_server.sockets[0].getsockname()[1]
        self.env = dict(os.environ, DEN_URL=f"http://127.0.0.1:{port}", DEN_TOKEN="fake-den-token",
                        RCON_HOST="fake-host", RCON_PORT="12345", RCON_PASSWORD="fake-rcon-secret",
                        MCRCON=str(self.bin / "mcrcon"), SYSTEMCTL=str(self.bin / "systemctl"),
                        MC_SERVER_DIR=str(self.mc), MC_UNIT="fake-unit", MC_BACKUP_DIR=str(self.backups),
                        DEN_MINECRAFT_PLAYERS=str(self.mapping), MC_ADDRESS="mc.example.test",
                        DEN_SERVER_SLUG="friends", FAKE_LOG=str(self.log), FAKE_STATE=str(self.state))
        for action in ("START", "STOP", "RESTART"):
            self.env[f"MC_{action}"] = shlex.quote(str(self.bin / action.lower()))
        self.processes = []

    async def asyncTearDown(self):
        self.ws_server.close()
        await self.ws_server.wait_closed()
        for process in self.processes:
            if process.returncode is None:
                process.terminate()
                await asyncio.wait_for(process.wait(), 5)
            _, stderr = await process.communicate()
            self.assertNotIn(b"fake-den-token", stderr)
            self.assertNotIn(b"fake-rcon-secret", stderr)
        await asyncio.to_thread(self.http.shutdown)
        self.http.server_close()
        self.http_thread.join(timeout=2)

    async def launch(self, *args):
        process = await asyncio.create_subprocess_exec(sys.executable, str(RELAY), *args, env=self.env,
                                                       stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE)
        self.processes.append(process)
        return process

    async def frame(self, kind):
        frame = await asyncio.wait_for(self.frames.get(), 8)
        self.assertEqual(frame["type"], kind, frame)
        return frame

    def invocations(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []

    async def test_hello_and_first_status(self):
        await self.launch()
        ws = await asyncio.wait_for(self.connections.get(), 5)
        self.assertEqual(ws.request.path, "/servers/relay")
        self.assertEqual(ws.request.headers["Authorization"], "Bearer fake-den-token")
        info = (await self.frame("hello"))["server"]
        self.assertEqual(info, {
            "slug": "friends", "game": "minecraft", "name": "Friend world",
            "details": ["1.21.1", "Fabric", "2 mods"], "address": "mc.example.test",
            "icon_png": base64.b64encode(ICON).decode(), "actions": [
                {"id": "save", "label": "Save world", "admin_only": False, "states": ["up"]},
                {"id": "start", "label": "Start", "admin_only": False, "states": ["down", "asleep"]},
                {"id": "restart", "label": "Restart", "admin_only": True, "states": ["up", "starting"]},
                {"id": "stop", "label": "Stop", "admin_only": True, "states": ["up", "starting"]},
            ],
        })
        status = (await self.frame("status"))["status"]
        self.assertEqual(status["state"], "up")
        self.assertEqual(status["players"], [{"name": "Steve_1984", "user_id": "01DENUSER"},
                                             {"name": "Alex", "user_id": None}])
        self.assertEqual(status["max_players"], 5)
        stats = {s["key"]: s for s in status["stats"]}
        self.assertEqual(stats["tps"], {"key": "tps", "label": "TPS", "unit": "tps", "value": 20.0, "graph": True})
        self.assertEqual(stats["memory"]["unit"], "bytes")
        self.assertEqual(stats["memory"]["label"], "Memory")
        self.assertGreater(stats["memory"]["value"], 0)
        self.assertGreaterEqual(stats["uptime"]["value"], 0)
        self.assertEqual(stats["uptime"]["unit"], "seconds")
        self.assertEqual(stats["backup"]["value"], 1700000000)
        self.assertEqual(stats["backup"]["label"], "Last backup")
        self.assertEqual(stats["backup"]["unit"], "timestamp")
        self.assertNotIn("cpu", stats)
        self.assertEqual(status["playtime"], [{"name": "Steve_1984", "user_id": "01DENUSER", "total_seconds": 120},
                                              {"name": "Alex", "user_id": None, "total_seconds": 30}])
        self.assertEqual(self.http_requests, [("/users", "Bearer fake-den-token")])
        for invocation in self.invocations():
            if invocation["executable"] == "mcrcon":
                self.assertTrue(invocation["rcon_env_ok"])
                self.assertNotIn("fake-rcon-secret", invocation["args"])
        repeated = await asyncio.wait_for(self.frames.get(), 12)
        self.assertEqual(repeated["type"], "status")
        self.assertEqual(repeated["status"]["players"], status["players"])
        self.assertNotIn("playtime", repeated["status"])
        self.assertEqual(self.http_requests, [("/users", "Bearer fake-den-token")])

    async def test_commands_and_ping(self):
        await self.launch()
        ws = await asyncio.wait_for(self.connections.get(), 5)
        await self.frame("hello")
        await self.frame("status")
        self.state.write_text('{"say_fail": true}')
        await ws.send(json.dumps({"type": "command", "command_id": "restart-1", "action": "restart", "by": "nerc"}))
        async with asyncio.timeout(5):
            while not any(i["executable"] == "restart" for i in self.invocations()):
                await asyncio.sleep(0.01)
        pong = await ws.ping()
        await asyncio.wait_for(pong, 0.3)
        self.assertEqual(await self.frame("result"), {"type": "result", "command_id": "restart-1", "ok": True, "message": "Restarting"})
        status = (await self.frame("status"))["status"]
        self.assertNotIn("playtime", status)
        cpu = next(s for s in status["stats"] if s["key"] == "cpu")
        self.assertEqual((cpu["label"], cpu["unit"]), ("CPU", "percent"))
        self.assertEqual(cpu["value"], round(cpu["value"], 1))
        log = self.invocations()
        say = next(index for index, invocation in enumerate(log) if invocation["args"] == ["-c", "say Restarting now (nerc)"])
        restart = next(index for index, invocation in enumerate(log) if invocation["executable"] == "restart")
        self.assertLess(say, restart)
        self.assertEqual(len(self.http_requests), 1)
        for action, message in (("unknown", "Unknown action"), ("save", "World saved"), ("start", "Starting"), ("stop", "Stopping")):
            await ws.send(json.dumps({"type": "command", "command_id": action, "action": action, "by": "nerc"}))
            self.assertEqual(await self.frame("result"), {"type": "result", "command_id": action, "ok": action != "unknown", "message": message})
            await self.frame("status")
        self.assertIn({"executable": "mcrcon", "args": ["-c", "save-all flush"], "rcon_env_ok": True}, self.invocations())
        self.state.write_text('{"command_fail": "stop"}')
        await ws.send(json.dumps({"type": "command", "command_id": "failed", "action": "stop", "by": "nerc"}))
        self.assertEqual(await self.frame("result"), {"type": "result", "command_id": "failed", "ok": False, "message": ("failure:" + "x" * 250)[:200]})
        await self.frame("status")
        # A command also causes a metadata check and an immediate fresh status.
        (self.mc / "server.properties").write_text("motd=§bChanged\nlevel-name=custom-world\n")
        await ws.send(json.dumps({"type": "command", "command_id": "changed", "action": "save", "by": "nerc"}))
        await self.frame("result")
        self.assertEqual((await self.frame("hello"))["server"]["name"], "Changed")
        await self.frame("status")
        self.assertEqual(self.connection_count, 1)

    async def test_reconnect(self):
        await self.launch()
        ws = await asyncio.wait_for(self.connections.get(), 5)
        first = await self.frame("hello")
        status = await self.frame("status")
        await ws.close()
        await asyncio.wait_for(self.connections.get(), 5)
        self.assertEqual(await self.frame("hello"), first)
        self.assertEqual((await self.frame("status"))["status"]["playtime"], status["status"]["playtime"])
        self.assertEqual(self.connection_count, 2)

    async def test_once(self):
        process = await self.launch("--once")
        stdout, stderr = await asyncio.wait_for(process.communicate(), 5)
        self.assertEqual(process.returncode, 0, stderr.decode())
        frames = [json.loads(line) for line in stdout.splitlines()]
        self.assertEqual([f["type"] for f in frames], ["hello", "status"])
        self.assertEqual(frames[0]["server"]["name"], "Friend world")
        self.assertEqual(frames[1]["status"]["state"], "up")
        self.assertEqual(self.connection_count, 0)
        self.assertEqual([path for path, _ in self.http_requests], ["/users"])
        self.assertFalse(any("activities" in path for path, _ in self.http_requests))

    async def test_states_and_unavailable_users(self):
        self.users_fail = True
        for active, list_fail, expected in (("active", True, "starting"), ("activating", False, "starting"),
                                             ("deactivating", False, "stopping"), ("inactive", False, "down")):
            with self.subTest(active=active):
                self.state.write_text(json.dumps({"active": active, "list_fail": list_fail}))
                process = await self.launch("--once")
                stdout, stderr = await asyncio.wait_for(process.communicate(), 5)
                self.assertEqual(process.returncode, 0, stderr.decode())
                status = json.loads(stdout.splitlines()[1])["status"]
                self.assertEqual(status["state"], expected)
                self.assertEqual(status["players"], [])
                self.assertEqual(status["max_players"], None)
                self.assertTrue(all(p["user_id"] is None for p in status["playtime"]))
                self.assertEqual(next(s["value"] for s in status["stats"] if s["key"] == "backup"), 1700000000)
        self.state.write_text("{}")
        process = await self.launch("--once")
        stdout, stderr = await asyncio.wait_for(process.communicate(), 5)
        self.assertEqual(process.returncode, 0, stderr.decode())
        status = json.loads(stdout.splitlines()[1])["status"]
        self.assertEqual(status["state"], "up")
        self.assertTrue(all(p["user_id"] is None for p in status["players"]))

    def test_parsers(self):
        loader = importlib.machinery.SourceFileLoader("minecraft_relay", str(RELAY))
        module = importlib.util.module_from_spec(importlib.util.spec_from_loader(loader.name, loader))
        loader.exec_module(module)
        self.assertEqual(module.parse_list("There are 2 of a max of 5 players online: a, b"), (["a", "b"], 5))
        self.assertEqual(module.parse_list("§aThere are 0 of a max of 5 players online: "), ([], 5))
        with self.assertRaises(ValueError):
            module.parse_list("RCON unavailable")
        self.assertEqual(module.parse_tps("§7TPS from last 5s, 10s, 1m:\n§a*19.75, 20.0, 20.0"), 19.75)
        self.assertEqual(module.parse_tps("TPS from last 5s:\n20.0"), 20.0)
        self.assertIsNone(module.parse_tps("spark is not installed"))


if __name__ == "__main__":
    unittest.main()
