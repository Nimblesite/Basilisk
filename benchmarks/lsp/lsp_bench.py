#!/usr/bin/env python3
"""A/B benchmark for the Basilisk LSP editing loop.

Drives a real `basilisk lsp` (stdio JSON-RPC) over a synthetic crossModule
workspace and times the scenarios where main and the salsa branch differ:

  scan      initialize -> startup diagnostics complete (all files published)
  rename    didChange on open lib.py renaming an export -> the dependent
            consumers' changed diagnostics republished
  restore   the inverse rename (same shape as `rename`, second sample)
  body      didChange on open lib.py editing a function body only (exports
            unchanged) -> own-file diagnostics published
  keystroke didChange on an open consumer file (body edit) -> own-file
            diagnostics published

Workspace shape: lib.py exporting FUNCS functions; half the files import lib
(`from lib import fK` + call), half are unrelated modules. All imports resolve.

Completion detection: every scenario knows the exact set of URIs the server
republishes and waits until each has been seen (with a hard timeout), so the
measured time is "last relevant publish received", not a quiet-period guess.
"""

import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

FUNCS = 20
QUIET_TIMEOUT = 120.0  # hard per-scenario ceiling, seconds


class Lsp:
    def __init__(self, binary: str, cwd: str):
        # The server runs with the workspace as cwd; resolve the binary path
        # first so relative paths (e.g. target/release/basilisk) work.
        self.proc = subprocess.Popen(
            [os.path.abspath(binary), "lsp"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            cwd=cwd,
        )
        self.next_id = 1
        self.buf = b""

    def send(self, method: str, params, is_request: bool = True):
        msg = {"jsonrpc": "2.0", "method": method, "params": params}
        if is_request:
            msg["id"] = self.next_id
            self.next_id += 1
        raw = json.dumps(msg).encode()
        self.proc.stdin.write(b"Content-Length: %d\r\n\r\n%s" % (len(raw), raw))
        self.proc.stdin.flush()
        return msg.get("id")

    def read_msg(self, timeout: float):
        """Read one framed message; returns None on timeout."""
        deadline = time.monotonic() + timeout
        header_end = self.buf.find(b"\r\n\r\n")
        while header_end < 0:
            if time.monotonic() > deadline:
                return None
            chunk = self._read_some(deadline)
            if chunk is None:
                return None
            self.buf += chunk
            header_end = self.buf.find(b"\r\n\r\n")
        headers = self.buf[:header_end].decode()
        length = 0
        for line in headers.split("\r\n"):
            if line.lower().startswith("content-length:"):
                length = int(line.split(":")[1].strip())
        body_start = header_end + 4
        while len(self.buf) < body_start + length:
            if time.monotonic() > deadline:
                return None
            chunk = self._read_some(deadline)
            if chunk is None:
                return None
            self.buf += chunk
        body = self.buf[body_start : body_start + length]
        self.buf = self.buf[body_start + length :]
        return json.loads(body)

    def _read_some(self, deadline: float):
        import select

        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return None
        r, _, _ = select.select([self.proc.stdout], [], [], remaining)
        if not r:
            return None
        chunk = os.read(self.proc.stdout.fileno(), 65536)
        return chunk if chunk else None

    def wait_for_publishes(self, want_uris: set, timeout: float) -> float:
        """Consume messages until every URI in want_uris has been published.

        Returns the monotonic time of the LAST relevant publish."""
        pending = set(want_uris)
        last_t = time.monotonic()
        deadline = time.monotonic() + timeout
        while pending and time.monotonic() < deadline:
            msg = self.read_msg(timeout=deadline - time.monotonic())
            if msg is None:
                break
            if msg.get("method") == "textDocument/publishDiagnostics":
                uri = msg["params"]["uri"]
                if uri in pending:
                    pending.discard(uri)
                    last_t = time.monotonic()
        if pending:
            raise TimeoutError(
                f"{len(pending)} URIs never published, e.g. {sorted(pending)[:3]}"
            )
        return last_t

    def drain(self, quiet: float = 0.5):
        """Consume messages until the pipe is quiet for `quiet` seconds."""
        while True:
            msg = self.read_msg(timeout=quiet)
            if msg is None:
                return

    def shutdown(self):
        try:
            self.send("shutdown", None)
            self.send("exit", None, is_request=False)
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()


def lib_source(rename_k: int | None = None, body_v: int = 1) -> str:
    lines = []
    for k in range(FUNCS):
        name = f"f{k}_renamed" if rename_k == k else f"f{k}"
        lines.append(f"def {name}(x: int) -> int:\n    return x + {body_v}\n")
    return "\n".join(lines)


IMPORTER_EVERY = int(os.environ.get("IMPORTER_EVERY", "2"))


def make_workspace(root: Path, n_files: int) -> dict:
    (root / "lib.py").write_text(lib_source())
    uris = {"lib": (root / "lib.py").as_uri()}
    consumers, unrelated, affected = [], [], []
    for i in range(n_files):
        if i % IMPORTER_EVERY == 0:
            p = root / f"consumer_{i}.py"
            k = i % FUNCS
            p.write_text(
                f"import lib\n\ndef use_{i}(v: int) -> int:\n    return f{k}(v)\n"
            )
            consumers.append(p.as_uri())
            if k == 0:
                affected.append(p.as_uri())
        else:
            p = root / f"module_{i}.py"
            p.write_text(
                f"def local_{i}(v: int) -> int:\n    return v * {i}\n\n"
                f"VALUE_{i}: int = {i}\n"
            )
            unrelated.append(p.as_uri())
    uris["consumers"] = consumers
    uris["unrelated"] = unrelated
    # Consumers whose diagnostics actually change when `f0` is renamed away —
    # the completion signal for the rename/restore scenarios.
    uris["affected"] = affected
    uris["all"] = [uris["lib"]] + consumers + unrelated
    return uris


def did_change(lsp: Lsp, uri: str, version: int, text: str):
    lsp.send(
        "textDocument/didChange",
        {
            "textDocument": {"uri": uri, "version": version},
            "contentChanges": [{"text": text}],
        },
        is_request=False,
    )


def bench(binary: str, n_files: int) -> dict:
    root = Path(tempfile.mkdtemp(prefix=f"bsk_lspbench_{n_files}_"))
    uris = make_workspace(root, n_files)
    all_set = set(uris["all"])
    results = {}
    lsp = Lsp(binary, cwd=str(root))
    try:
        # -- initialize / startup scan ------------------------------------
        t0 = time.monotonic()
        init_id = lsp.send(
            "initialize",
            {
                "processId": None,
                "rootUri": root.as_uri(),
                "capabilities": {},
                "trace": "off",
                "initializationOptions": {"analysisMode": "crossModule"},
            },
        )
        # The server must acknowledge initialize BEFORE `initialized` is sent,
        # or tower-lsp drops the premature notification (and never scans).
        while True:
            msg = lsp.read_msg(timeout=30)
            if msg is None:
                raise TimeoutError("no response to initialize")
            if msg.get("id") == init_id:
                break
        lsp.send("initialized", {}, is_request=False)
        last = lsp.wait_for_publishes(all_set, timeout=QUIET_TIMEOUT)
        results["scan"] = last - t0
        lsp.drain(quiet=1.0)  # let any trailing re-publishes settle

        # -- open lib.py ---------------------------------------------------
        lsp.send(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": uris["lib"],
                    "languageId": "python",
                    "version": 1,
                    "text": lib_source(),
                }
            },
            is_request=False,
        )
        lsp.wait_for_publishes({uris["lib"]}, timeout=30)
        lsp.drain(quiet=1.0)

        # -- export rename: dependent refresh ------------------------------
        # Completion = the consumers whose diagnostics change have republished
        # (names_undefined appears for the renamed export). Fair to both
        # full-republish and changed-only servers; the trailing drain absorbs
        # any additional publishes outside the timed window.
        affected_set = set(uris["affected"])
        t0 = time.monotonic()
        did_change(lsp, uris["lib"], 2, lib_source(rename_k=0))
        last = lsp.wait_for_publishes(affected_set, timeout=QUIET_TIMEOUT)
        results["rename"] = last - t0
        lsp.drain(quiet=1.0)

        # -- restore (second export-change sample) --------------------------
        t0 = time.monotonic()
        did_change(lsp, uris["lib"], 3, lib_source())
        last = lsp.wait_for_publishes(affected_set, timeout=QUIET_TIMEOUT)
        results["restore"] = last - t0
        lsp.drain(quiet=1.0)

        # -- body-only edit on lib.py (exports unchanged) -------------------
        samples = []
        for v in (2, 3, 4):
            t0 = time.monotonic()
            did_change(lsp, uris["lib"], 3 + v, lib_source(body_v=v))
            last = lsp.wait_for_publishes({uris["lib"]}, timeout=30)
            samples.append(last - t0)
            lsp.drain(quiet=0.3)
        results["body"] = sorted(samples)[1]  # median of 3

        # -- keystrokes on an open consumer ---------------------------------
        consumer = uris["consumers"][0]
        base = "import lib\n\ndef use_0(v: int) -> int:\n    return f0(v)\n"
        lsp.send(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": consumer,
                    "languageId": "python",
                    "version": 1,
                    "text": base,
                }
            },
            is_request=False,
        )
        lsp.wait_for_publishes({consumer}, timeout=30)
        lsp.drain(quiet=0.5)
        samples = []
        for v in range(2, 7):
            # Body-only edit — typing inside the function; exports unchanged.
            text = base.replace("return f0(v)", f"return f0(v + {v})")
            t0 = time.monotonic()
            did_change(lsp, consumer, v, text)
            last = lsp.wait_for_publishes({consumer}, timeout=30)
            samples.append(last - t0)
            lsp.drain(quiet=0.2)
        results["keystroke"] = sorted(samples)[len(samples) // 2]
    finally:
        lsp.shutdown()
    return results


def main():
    binary = sys.argv[1]
    n_files = int(sys.argv[2])
    label = sys.argv[3] if len(sys.argv) > 3 else binary
    r = bench(binary, n_files)
    out = {"label": label, "n_files": n_files, **{k: round(v, 4) for k, v in r.items()}}
    print(json.dumps(out))


if __name__ == "__main__":
    main()
