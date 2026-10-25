#!/usr/bin/env python3
"""Report how rust-analyzer's completions through the `fprime_macros` expansions changed.

rust-analyzer expands a macro twice per keystroke, with and without `raCompletionMarker` at
the cursor, and the two must agree up to the cursor. `expansions_agree_up_to_the_cursor`
checks that agreement; this checks what the editor then offers.

It compares against a blessed baseline (`--bless`), not a quality bar, so it answers "did
this change what the editor offers, and where" — not "does it feel right". The baseline
depends on the rust-analyzer version, so re-bless after updating the extension.

Each case edits a real file through a `didOpen` overlay (nothing is written to disk) and
asks for completions at one cursor. The first two cases are controls, so a server that is
not answering reads as inconclusive rather than as a regression. Settings mirror
`fprime_wasm/templates/vscode-settings.json.tmpl`.

Usage:
    python3 tools/completion_probe.py            # compare against the baseline
    python3 tools/completion_probe.py --verbose  # and list the prefix-matching items
    python3 tools/completion_probe.py --bless    # record the current behaviour
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CASES = Path(__file__).resolve().parent / "completion_cases.json"
BASELINE = Path(__file__).resolve().parent / "completion_baseline.json"

# The rust-analyzer the editor actually uses. `~/.cargo/bin/rust-analyzer` is a rustup
# proxy that may not resolve to a real server.
SERVER_GLOBS = [
    Path.home() / ".vscode/extensions",
    Path.home() / ".vscode-server/extensions",
    Path.home() / ".vscode-insiders/extensions",
]

INDEX_TIMEOUT_S = 300

# Where an inserted body wants the cursor.
CURSOR = "<|>"

# Mirrors `fprime_wasm/templates/vscode-settings.json.tmpl`, in the nested form LSP wants.
# Without `allTargets` and `wasm`, the sequence bins do not load.
OPTIONS = {
    "cargo": {"allTargets": True, "features": ["wasm"]},
    "procMacro": {"enable": True},
    "checkOnSave": {"enable": False},
    "completion": {
        "autoAwait": {"enable": False},
        "addColonsToModule": False,
        "excludeTraits": [
            "core::convert::TryInto",
            "core::convert::Into",
            "core::any::Any",
            "alloc::borrow::ToOwned",
            "core::cmp::PartialEq",
            "core::clone::Clone",
        ],
        "callable": {"snippets": "fill_arguments"},
        "autoself": {"enable": False},
        "postfix": {"enable": False},
        "autoimport": {"enable": False},
    },
}


def find_server() -> Path:
    for root in SERVER_GLOBS:
        if not root.is_dir():
            continue
        for extension in sorted(root.glob("rust-lang.rust-analyzer-*")):
            binary = extension / "server" / "rust-analyzer"
            if binary.is_file() and os.access(binary, os.X_OK):
                return binary
    print(
        "completion_probe: no rust-analyzer found under "
        + ", ".join(str(root) for root in SERVER_GLOBS)
        + "\n  Install the VS Code extension; the rustup proxy at ~/.cargo/bin is not a"
        " server.",
        file=sys.stderr,
    )
    sys.exit(2)


class Client:
    """Barely enough LSP to ask what completes at a cursor."""

    def __init__(self, server: Path) -> None:
        self.process = subprocess.Popen(
            [str(server)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            cwd=REPO,
        )
        self.next_id = 1
        self.replies: dict[int, dict] = {}
        self.quiescent = threading.Event()
        self.lock = threading.Lock()
        threading.Thread(target=self._read_loop, daemon=True).start()

    def _send(self, message: dict) -> None:
        body = json.dumps(message).encode()
        assert self.process.stdin is not None
        self.process.stdin.write(f"Content-Length: {len(body)}\r\n\r\n".encode() + body)
        self.process.stdin.flush()

    def _read_loop(self) -> None:
        stream = self.process.stdout
        assert stream is not None
        while True:
            length = None
            while True:
                line = stream.readline()
                if not line:
                    return
                if line in (b"\r\n", b"\n"):
                    break
                name, _, value = line.decode(errors="replace").partition(":")
                if name.strip().lower() == "content-length":
                    length = int(value.strip())
            if length is None:
                continue
            payload = json.loads(stream.read(length))

            if payload.get("method") == "experimental/serverStatus":
                if payload.get("params", {}).get("quiescent"):
                    self.quiescent.set()
            elif "id" in payload and "method" not in payload:
                with self.lock:
                    self.replies[payload["id"]] = payload
            elif "id" in payload and "method" in payload:
                # A server-to-client request; answer nothing so it does not block.
                self._send({"jsonrpc": "2.0", "id": payload["id"], "result": None})

    def request(self, method: str, params: dict, timeout: float = 90.0) -> dict:
        with self.lock:
            request_id = self.next_id
            self.next_id += 1
        self._send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        deadline = time.time() + timeout
        while time.time() < deadline:
            with self.lock:
                if request_id in self.replies:
                    return self.replies.pop(request_id)
            if self.process.poll() is not None:
                raise RuntimeError("rust-analyzer exited")
            time.sleep(0.02)
        raise TimeoutError(f"{method} timed out")

    def notify(self, method: str, params: dict) -> None:
        self._send({"jsonrpc": "2.0", "method": method, "params": params})

    def initialize(self) -> None:
        self.request(
            "initialize",
            {
                "processId": os.getpid(),
                "rootUri": REPO.as_uri(),
                "capabilities": {
                    "textDocument": {
                        "completion": {
                            "completionItem": {
                                "snippetSupport": False,
                                "insertReplaceSupport": True,
                            },
                            "contextSupport": True,
                        }
                    },
                    "experimental": {"serverStatusNotification": True},
                },
                "initializationOptions": OPTIONS,
            },
        )
        self.notify("initialized", {})

    def open(self, path: Path, text: str) -> None:
        """Opens `path` with `text` as its contents — an overlay, not a write."""
        self.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": path.as_uri(),
                    "languageId": "rust",
                    "version": 1,
                    "text": text,
                }
            },
        )

    def completion(self, path: Path, line: int, character: int) -> list[dict]:
        reply = self.request(
            "textDocument/completion",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": {"line": line, "character": character},
                "context": {"triggerKind": 1},
            },
        )
        result = reply.get("result") or []
        return result.get("items", []) if isinstance(result, dict) else result

    def shutdown(self) -> None:
        try:
            self.request("shutdown", {}, timeout=10)
            self.notify("exit", {})
        except Exception:
            pass
        self.process.terminate()


class Case:
    """One cursor placed `keep` characters into an identifier.

    * `"truncate"` — the rest of the name is deleted: typing something new. The name does
      not resolve.
    * `"inside"` — the name is kept whole with the cursor in it: editing an existing one.
      The name resolves with the marker inside it, so only this catches a dropped marker.
    * `"insert"` — `insert` is added after the anchor line, with `<|>` for the cursor; for a
      position no existing line holds, such as nothing typed yet.
    """

    def __init__(self, spec: dict) -> None:
        self.why: str = spec["why"]
        self.path = REPO / spec["path"]
        self.anchor: str = spec["anchor"]
        self.name: str = spec.get("name", "")
        self.keep: str = spec.get("keep", "")
        self.expect: str = spec["expect"]
        self.occurrence: int = spec.get("occurrence", 1)
        self.insert: str = spec.get("insert", "")
        self.mode: str = "insert" if self.insert else spec.get("mode", "truncate")

    def locate(self, lines: list[str]) -> int:
        """The index of the line this case truncates."""
        seen = 0
        for number, line in enumerate(lines):
            if self.anchor in line and self.name in line:
                seen += 1
                if seen == self.occurrence:
                    return number
        raise LookupError(f"{self.path}: no line {self.occurrence} containing {self.anchor!r}")


def overlay(path: Path, cases: list[Case]) -> tuple[str, dict[int, tuple[int, int]]]:
    """`path`'s text with every case for it applied at once, and each case's cursor.

    The other cases' edits are in other statements, so they cannot reach the expansion the
    cursor is in.
    """
    original = path.read_text().splitlines()
    lines = list(original)
    cursors: dict[int, tuple[int, int]] = {}

    # Located against the untouched text, so one case's edit cannot hide another's anchor.
    located = [(index, case, case.locate(original)) for index, case in enumerate(cases)]

    # Two cases may share a line only if at most one rewrites it; an insertion goes after it.
    for kind in ("edit", "insert"):
        taken: dict[int, str] = {}
        for _, case, number in located:
            if (case.mode == "insert") != (kind == "insert"):
                continue
            if number in taken:
                raise ValueError(
                    f"{path}:{number + 1} is the line for both {taken[number]!r} and "
                    f"{case.why!r}; two cases cannot {kind} one line"
                )
            taken[number] = case.why

    for index, case, number in located:
        if case.mode == "insert":
            continue
        line = original[number]
        column = line.index(case.name) + len(case.keep)
        if case.mode == "truncate":
            lines[number] = line.replace(case.name, case.keep, 1)
        cursors[index] = (number, column)

    # Insertions last, shallowest first, shifting every cursor at or below each one.
    shift = 0
    for index, case, number in sorted(located, key=lambda found: found[2]):
        if case.mode != "insert":
            continue

        written = case.insert.split("\n")
        at = next((offset for offset, line in enumerate(written) if CURSOR in line), None)
        if at is None:
            raise ValueError(f"{case.why!r}: an inserted body needs {CURSOR} for the cursor")

        column = written[at].index(CURSOR)
        written[at] = written[at].replace(CURSOR, "", 1)

        into = number + 1 + shift
        lines[into:into] = written
        for other, (line, other_column) in cursors.items():
            if line >= into:
                cursors[other] = (line + len(written), other_column)
        cursors[index] = (into + at, column)
        shift += len(written)

    text = "\n".join(lines) + "\n"
    verify(path, cases, text, cursors)

    return text, cursors


def verify(path: Path, cases: list[Case], text: str, cursors: dict[int, tuple[int, int]]) -> None:
    """Every cursor sits where its case meant it to.

    A cursor a few lines off returns every name in scope, which looks exactly like the macro
    being broken.
    """
    lines = text.split("\n")

    for index, case in enumerate(cases):
        line, column = cursors[index]
        found = lines[line]
        wanted = None
        if case.mode == "insert":
            body = case.insert.split("\n")
            wanted = next(line for line in body if CURSOR in line).replace(CURSOR, "")

        if wanted is not None and found != wanted:
            raise ValueError(
                f"{case.why!r}: the cursor landed on {found!r}, not on {wanted!r} — "
                f"{path.name}:{line + 1} is the wrong line"
            )
        if wanted is None and case.keep not in found:
            raise ValueError(
                f"{case.why!r}: the cursor landed on {found!r}, which does not hold "
                f"{case.keep!r} — {path.name}:{line + 1} is the wrong line"
            )


def inserted(item: dict) -> str:
    """The text taking the item would write — not its `label`, which for a generated
    `const` includes its whole value."""
    edit = item.get("textEdit") or {}
    return edit.get("newText") or item.get("insertText") or item.get("label", "")


def ranked(items: list[dict]) -> list[dict]:
    """The list in the order an editor would show it."""
    return sorted(items, key=lambda item: (item.get("sortText") or item.get("label", "")))


def measure(items: list[dict], case: Case) -> dict:
    """What the list says about `case.expect`: where it ranks as itself (`bare_rank`) and as a
    path ending in it, like `Ref::Choice::RED` (`qualified_rank`)."""
    bare_rank = None
    qualified_rank = None
    found = None

    for position, item in enumerate(ranked(items), start=1):
        text = inserted(item)
        if text == case.expect:
            if bare_rank is None:
                bare_rank, found = position, item
        elif text.endswith(f"::{case.expect}") and qualified_rank is None:
            qualified_rank = position
            found = found or item

    return {
        # 0 when the cursor maps to nothing: the two expansions disagree before it.
        "items": len(items),
        "bare_rank": bare_rank,
        "qualified_rank": qualified_rank,
        "import": found is not None and bool(found.get("additionalTextEdits")),
    }


def main() -> int:
    verbose = "--verbose" in sys.argv
    bless = "--bless" in sys.argv

    # A stale proc-macro dylib answers for the macro as it was, not as it is.
    print("completion_probe: cargo check (so the proc macro is current)", file=sys.stderr)
    for args in (
        ["cargo", "check", "-p", "example", "--target", "wasm32v1-none", "--features", "wasm"],
        ["cargo", "check", "-p", "example", "--tests"],
    ):
        result = subprocess.run(args, cwd=REPO, capture_output=True)
        if result.returncode != 0:
            print(result.stderr.decode()[-3000:], file=sys.stderr)
            print(f"completion_probe: `{' '.join(args)}` failed", file=sys.stderr)
            return 2

    cases = [Case(spec) for spec in json.loads(CASES.read_text())]
    by_file: dict[Path, list[Case]] = {}
    for case in cases:
        by_file.setdefault(case.path, []).append(case)

    client = Client(find_server())
    try:
        client.initialize()

        positions: dict[int, tuple[Path, int, int]] = {}
        for path, group in by_file.items():
            text, cursors = overlay(path, group)
            client.open(path, text)
            for local, (line, column) in cursors.items():
                positions[cases.index(group[local])] = (path, line, column)

        print("completion_probe: waiting for the project to load", file=sys.stderr)
        if not client.quiescent.wait(INDEX_TIMEOUT_S):
            print("completion_probe: rust-analyzer never went quiescent", file=sys.stderr)
            return 2

        measured = {}
        for index, case in enumerate(cases):
            path, line, column = positions[index]
            items = client.completion(path, line, column)
            measured[case.why] = measure(items, case)
            if verbose:
                print(f"  {case.why}: {case.keep}|  ({len(items)} items)")
                for item in ranked(items):
                    text = inserted(item)
                    if text == case.expect or text.endswith(f"::{case.expect}"):
                        print(f"      {text}")

        # Without both controls, nothing else measured means anything.
        for case in cases[:2]:
            if measured[case.why]["bare_rank"] is None:
                print(
                    f"completion_probe: INCONCLUSIVE — the control {case.why!r} did not\n"
                    "  offer its own name, so the server is not answering.",
                    file=sys.stderr,
                )
                return 2
    finally:
        client.shutdown()

    if bless:
        BASELINE.write_text(json.dumps(measured, indent=2, sort_keys=True) + "\n")
        print(f"completion_probe: blessed {len(measured)} cases into {BASELINE.name}")
        return 0

    if not BASELINE.is_file():
        print(
            f"completion_probe: no {BASELINE.name}; run once with --bless to record the"
            " current behaviour.",
            file=sys.stderr,
        )
        return 2

    baseline = json.loads(BASELINE.read_text())
    changed = 0
    for case in cases:
        was, now = baseline.get(case.why), measured[case.why]
        ok = was == now
        changed += not ok
        print(f"  {'ok  ' if ok else 'FAIL'} {case.why}")
        if not ok:
            print(f"       was {json.dumps(was, sort_keys=True)}")
            print(f"       now {json.dumps(now, sort_keys=True)}")

    print(f"\ncompletion_probe: {len(cases) - changed}/{len(cases)} unchanged")
    if changed:
        print(
            "\n`items` dropping to 0 matters most: the two expansions no longer agree up to the\n"
            "cursor, or a marker-bearing name lost its marker on the way out.\n"
            "`bare_rank` going null means the name is only offered by its full path.\n"
            "\nIf the change is deliberate, re-bless with --bless and say so in the commit.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
