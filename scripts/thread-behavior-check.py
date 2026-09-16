#!/usr/bin/env python3
"""Thread behaviour before/after check against a real den-server process.

One fixed expectation set, run unchanged against any server binary. Every check
records name, expected, observed and passed; `--label` is metadata only and
changes nothing. A correct server passes; the contract-only server fails the
thread checks and passes the legacy ones. Checks that genuinely cannot run are
marked blocked, and only when their precondition is found missing at runtime.

  python3 thread-behavior-check.py --binary path/to/den-server --port 17036 \
      --out result.json --label before

Starts its own server on its own loopback port over a temporary directory it
creates and owns, restarts that process for real against the same database, and
stops it in a finally. Standard library only. Records no credentials.

Out of scope: object cards, unread and read-state accounting, follow.
"""
import argparse
import hashlib
import json
import os
import shutil
import signal
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone

PASSWORD = "smoke-password-123"  # throwaway instance; never recorded


def now():
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def port_in_use(port):
    with socket.socket() as s:
        s.settimeout(0.5)
        return s.connect_ex(("127.0.0.1", port)) == 0


def request(method, url, token=None, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    if data is not None:
        req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return r.status, decode(r.read())
    except urllib.error.HTTPError as e:
        return e.code, decode(e.read())


def decode(raw):
    text = raw.decode("utf-8", "replace")
    try:
        return json.loads(text)
    except ValueError:
        return text.strip()[:200]


def thread_of(message):
    """The thread a message belongs to, by either of the contract's two fields."""
    if not isinstance(message, dict):
        return None
    if message.get("thread_id"):
        return message["thread_id"]
    summary = message.get("thread")
    if isinstance(summary, dict):
        return summary.get("id")
    return None


class Server:
    def __init__(self, binary, port, data):
        self.binary, self.port, self.data = binary, port, data
        self.origin = f"http://127.0.0.1:{port}"
        self.proc = None
        self.starts = []

    def start(self):
        env = dict(os.environ)
        env.update(
            DEN_DB=os.path.join(self.data, "den.db"),
            DEN_UPLOADS=os.path.join(self.data, "uploads"),
            DEN_BOOTSTRAP_FILE=os.path.join(self.data, "bootstrap.key"),
            DEN_BIND=f"127.0.0.1:{self.port}",
            DEN_ORIGIN=self.origin,
        )
        log = open(os.path.join(self.data, f"server-{len(self.starts)}.log"), "wb")
        self.proc = subprocess.Popen(
            [self.binary], env=env, stdout=log, stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL)
        deadline = time.time() + 60
        while time.time() < deadline:
            if self.proc.poll() is not None:
                raise SystemExit(f"server exited during startup, rc={self.proc.returncode}")
            try:
                if request("GET", self.origin + "/health")[0] == 200:
                    break
            except Exception:
                pass
            time.sleep(0.3)
        else:
            raise SystemExit("server never became healthy")
        record = {"pid": self.proc.pid, "started_at": now()}
        self.starts.append(record)
        return record

    def stop(self):
        """SIGINT is this binary's graceful path; escalate only if it hangs."""
        if self.proc is None or self.proc.poll() is not None:
            self.proc = None
            return {"already_exited": True}
        pid = self.proc.pid
        self.proc.send_signal(signal.SIGINT)
        try:
            rc, escalated = self.proc.wait(timeout=20), False
        except subprocess.TimeoutExpired:
            self.proc.kill()
            rc, escalated = self.proc.wait(timeout=20), True
        self.proc = None
        try:
            os.kill(pid, 0)
            alive = True
        except OSError:
            alive = False
        return {"pid": pid, "exit_code": rc, "escalated_to_sigkill": escalated,
                "pid_still_alive": alive, "port_still_listening": port_in_use(self.port),
                "stopped_at": now()}


class Checks:
    def __init__(self):
        self.items = []
        self.scenario = None

    def check(self, name, expected, observed, passed, request=None, response_body=None):
        item = {"scenario": self.scenario, "name": name, "expected": expected,
                "observed": observed, "passed": bool(passed), "request": request}
        if response_body is not None:
            item["response_body"] = response_body
        self.items.append(item)

    def blocked(self, name, expected, reason):
        self.items.append({"scenario": self.scenario, "name": name, "expected": expected,
                           "observed": None, "passed": None, "blocked": True, "reason": reason})


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--binary", required=True)
    ap.add_argument("--port", type=int, default=17036)
    ap.add_argument("--out", required=True)
    ap.add_argument("--label", default="", help="metadata only; affects nothing")
    ap.add_argument("--keep-data", action="store_true")
    args = ap.parse_args()

    binary = os.path.abspath(args.binary)
    if not os.access(binary, os.X_OK):
        raise SystemExit(f"not an executable: {binary}")
    if port_in_use(args.port):
        raise SystemExit(f"port {args.port} is already in use")
    data = tempfile.mkdtemp(prefix="den-thread-check-")  # created and owned here

    c = Checks()
    server = Server(binary, args.port, data)
    o = server.origin
    result = {
        "label": args.label,
        "captured_at": now(),
        "binary": {"path": binary, "size_bytes": os.path.getsize(binary),
                   "sha256": hashlib.sha256(open(binary, "rb").read()).hexdigest(),
                   "mtime": datetime.fromtimestamp(os.path.getmtime(binary),
                                                   timezone.utc).isoformat(timespec="seconds")},
        "isolation": {"origin": o, "port": args.port, "data_dir": data,
                      "note": "Private loopback port and a temporary directory this script created."},
        "process": {"starts": [], "restart": None, "final_stop": None},
        "out_of_scope": ["object cards", "unread and read-state accounting", "follow"],
    }

    try:
        result["process"]["starts"].append(server.start())
        result["server_health"] = request("GET", o + "/health")[1]
        status, session = request(
            "POST", o + "/auth/init",
            body={"username": "smoke", "password": PASSWORD,
                  "bootstrap_token": open(os.path.join(data, "bootstrap.key")).read(),
                  "display_name": "Smoke"})
        if status != 200:
            raise SystemExit(f"bootstrap failed: {status} {session}")
        token = session["token"]
        channel = next(ch["id"] for ch in request("GET", o + "/channels", token)[1]
                       if ch["kind"] == "text")
        result["identity"] = {"user_id": session["user"]["id"], "channel_id": channel,
                              "note": "One authenticated identity. No credentials recorded."}

        def post(body):
            st, m = request("POST", f"{o}/channels/{channel}/messages", token, body)
            return st, m, body

        def listing(query):
            st, rows = request("GET", f"{o}/channels/{channel}/messages?{query}", token)
            ids = [m["id"] for m in rows] if isinstance(rows, list) else None
            return st, rows, ids

        # ------------------------------------------------ a reply creates the conversation
        c.scenario = "reply_creates_a_thread"
        _, root, root_req = post({"content": "why is the build red"})
        root_id = root.get("id") if isinstance(root, dict) else None
        st1, reply1, req1 = post({"content": "it looks like a flaky test", "reply_to": root_id})
        st2, reply2, req2 = post({"content": "same one as yesterday",
                                  "reply_to": reply1.get("id") if isinstance(reply1, dict) else None})
        t1, t2 = thread_of(reply1), thread_of(reply2)
        # An expectation naming another result must stay readable when that result
        # is missing, rather than recording a bare null as the expectation.
        first_thread = t1 if t1 is not None else "the first reply's thread id"
        c.check("the first reply belongs to a thread", "a thread id", t1, t1 is not None, req1)
        c.check("a reply to a reply stays in the same thread", first_thread, t2,
                t1 is not None and t2 == t1, req2)

        st, rows, flat_ids = listing("limit=50")
        refetched = next((m for m in rows if isinstance(m, dict) and m.get("id") == root_id),
                         None) if isinstance(rows, list) else None
        c.check("the refetched root carries that thread's summary", first_thread,
                thread_of(refetched), t1 is not None and thread_of(refetched) == t1,
                {"GET": f"/channels/{{channel}}/messages?limit=50", "message": "<root id>"})
        # Requires the thread to exist, so this cannot pass by there being no thread.
        c.check("the root itself stays out of the thread",
                "a thread exists and the root's own thread_id is null",
                {"thread": t1, "root_thread_id": refetched.get("thread_id")
                 if isinstance(refetched, dict) else "<root not returned>"},
                t1 is not None and isinstance(refetched, dict)
                and refetched.get("thread_id") is None)
        expected_ids = [root_id, reply1.get("id"), reply2.get("id")]
        c.check("the flat listing returns exactly the three messages", expected_ids, flat_ids,
                flat_ids == expected_ids, {"GET": f"/channels/{{channel}}/messages?limit=50"})
        st, _, root_ids = listing("limit=50&roots_only=true")
        c.check("roots_only returns the root alone", [root_id], root_ids, root_ids == [root_id],
                {"GET": f"/channels/{{channel}}/messages?limit=50&roots_only=true"})

        # ---------------------------------------------------------------- agent task grouping
        c.scenario = "task_grouping"
        task_a, task_b = "smoke-job-a", "smoke-job-b"
        _, a1, a1_req = post({"content": "starting job A", "task_id": task_a})
        _, b1, b1_req = post({"content": "starting job B", "task_id": task_b})
        ta, tb = thread_of(a1), thread_of(b1)
        c.check("task A's first message supplies a thread", "a thread id", ta, ta is not None, a1_req)
        c.check("task B's first message supplies a thread", "a thread id", tb, tb is not None, b1_req)
        c.check("two tasks from one identity get different threads", "two distinct thread ids",
                {"task_a": ta, "task_b": tb}, ta is not None and tb is not None and ta != tb)
        _, a2, a2_req = post({"content": "job A is halfway", "task_id": task_a})
        task_a_thread = ta if ta is not None else "task A's thread id"
        c.check("later progress rejoins its own task's thread", task_a_thread, thread_of(a2),
                ta is not None and thread_of(a2) == ta, a2_req)

        # --------------------------------------------------------- a real process restart
        c.scenario = "restart"
        _, _, before_ids = listing("limit=200")
        stopped = server.stop()
        started = server.start()
        result["process"]["starts"].append(started)
        result["process"]["restart"] = {"stop": stopped, "start": started,
                                        "same_database": os.path.join(data, "den.db")}
        _, _, after_ids = listing("limit=200")
        c.check("the restarted process serves the identical history", before_ids, after_ids,
                before_ids is not None and after_ids == before_ids,
                {"restart": {"stopped_pid": stopped.get("pid"), "exit_code": stopped.get("exit_code"),
                             "new_pid": started["pid"]}})
        _, a3, a3_req = post({"content": "job A resumed after a restart", "task_id": task_a})
        c.check("the same task ID continues its thread after the restart", task_a_thread,
                thread_of(a3), ta is not None and thread_of(a3) == ta, a3_req)

        # ------------------------------------------------------------- resolve and reopen
        c.scenario = "resolve_and_reopen"
        if ta is None:
            why = "task A supplied no thread id, so there is no thread to resolve"
            c.blocked("resolving task A's thread returns 200", 200, why)
            c.blocked("a delayed post to the resolved task returns 409", 409, why)
            c.blocked("reopening task A's thread returns 200", 200, why)
            c.blocked("a post after reopening returns 200", 200, why)
            c.blocked("that post is in task A's original thread", "task A's thread id", why)
        else:
            body = {"resolved": True}
            st, resolved = request("PATCH", f"{o}/threads/{ta}", token, body)
            c.check("resolving task A's thread returns 200", 200, st, st == 200,
                    {"PATCH": "/threads/{task A thread}", "body": body}, resolved)
            st, late, late_req = post({"content": "a late agent update for job A", "task_id": task_a})
            c.check("a delayed post to the resolved task returns 409", 409, st, st == 409,
                    late_req, late)
            body = {"resolved": False}
            st, reopened = request("PATCH", f"{o}/threads/{ta}", token, body)
            c.check("reopening task A's thread returns 200", 200, st, st == 200,
                    {"PATCH": "/threads/{task A thread}", "body": body}, reopened)
            st, a4, a4_req = post({"content": "job A continues after reopen", "task_id": task_a})
            c.check("a post after reopening returns 200", 200, st, st == 200, a4_req, a4)
            c.check("that post is in task A's original thread", ta, thread_of(a4),
                    thread_of(a4) == ta, a4_req)
    finally:
        result["process"]["final_stop"] = server.stop()
        counts = {}
        try:
            con = sqlite3.connect(f"file:{os.path.join(data, 'den.db')}?mode=ro", uri=True)
            for key, q in (("schema_version", "SELECT max(version) FROM _sqlx_migrations"),
                           ("threads", "SELECT count(*) FROM threads"),
                           ("thread_read_state", "SELECT count(*) FROM thread_read_state"),
                           ("thread_tasks", "SELECT count(*) FROM thread_tasks"),
                           ("messages", "SELECT count(*) FROM messages"),
                           ("messages_with_thread_id",
                            "SELECT count(*) FROM messages WHERE thread_id IS NOT NULL")):
                counts[key] = con.execute(q).fetchone()[0]
            con.close()
        except sqlite3.Error as e:
            counts["error"] = str(e)
        result["storage_counts"] = counts
        result["checks"] = c.items
        result["summary"] = {
            "total": len(c.items),
            "passed": sum(1 for x in c.items if x["passed"] is True),
            "failed": sum(1 for x in c.items if x["passed"] is False),
            "blocked": sum(1 for x in c.items if x.get("blocked")),
        }
        with open(args.out, "w") as f:
            json.dump(result, f, indent=2)
            f.write("\n")
        if not args.keep_data:
            shutil.rmtree(data, ignore_errors=True)  # only the directory this script created
        print(f"wrote {args.out}")
        print("summary:", json.dumps(result["summary"]))
        print("storage:", json.dumps(counts))
        print("stopped:", json.dumps(result["process"]["final_stop"]))
    return 0 if result["summary"]["failed"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
