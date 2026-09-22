#!/usr/bin/env python3
"""Check that argument completion still works through the `fprime_macros` expansions.

rust-analyzer expands a macro twice at every keystroke: once with `raCompletionMarker`
where the cursor is, and once without. It then finds the cursor in the real expansion at
the *raw byte offset* the marker sits at in the fake one. So the two expansions have to
agree up to the cursor — after it they may differ freely — and an expansion that inserts
so much as one token ahead of the cursor moves the offset the editor maps through and the
completion list silently degrades.

Nothing about that is visible to `cargo build`, to a token-string test, or to
`goto_probe.py`. `fprime_macros`' own unit tests can prove the two expansions agree
byte-for-byte (see `expansions_agree_up_to_the_cursor`), which is the cheap half. This is
the other half: whether the editor, given those expansions, actually offers the name.

**This compares against a blessed baseline rather than an absolute bar.** The known fix
for ranking bare constants — a per-statement glob scope, so a constant reachable only by
its full path is not the only thing offered — lives on a branch elsewhere and is not in
this repo. Demanding a quality bar this tree has never met would make the probe fail
always and mean nothing; recording what it does today and failing on *divergence* is what
makes it useful during a refactor. Bless with `--bless`, same idea as `FPRIME_UPDATE_REF`.

Each case truncates one identifier in a real file to a prefix, puts the cursor after that
prefix, and asks for completions. The edit is a `didOpen` overlay — nothing is written to
disk. Two controls come first: a plain-Rust one and one inside `#[fprime_main]` that is
not a dictionary name, so "the DSL broke completion" is separable from "macro expansion
broke completion" and from "the server was not ready".

Settings mirror `fprime_wasm/templates/vscode-settings.json.tmpl`, so what is measured is
what an author of a scaffolded project actually sees.

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
# `allTargets` and the `wasm` feature are both needed or the sequence bins do not load and
# every case inside one is inconclusive.
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

    `mode` is which half of the marker story the case is about:

    * `"truncate"` — the rest of the name is deleted, so the author is partway through
      typing something new. The name does not resolve, and what is measured is the scope
      the macro left around the cursor.
    * `"inside"` — the name is left whole and the cursor put in the middle of it, which is
      what going back to edit an existing argument looks like. The name *does* resolve,
      with the marker inside it, so this is the case that reaches `WrittenPath::emit`'s
      `in_progress` arm — and the only kind that can catch a marker being dropped.
    * `"insert"` — `insert` is written into the file just *after* the anchor line, with
      `<|>` where the cursor goes, and `name`/`keep` unused. For a position no existing line
      holds: nothing typed yet, or a control that needs a whole function of its own.
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
    """`path`'s text with every case's identifier truncated, and each case's cursor.

    Every case for a file is applied at once and the file is opened once. The other
    truncated names are left as ordinary unresolvable identifiers, which is what the DSL
    passes through anyway, and they are in other statements — so they cannot reach the
    expansion the cursor is in. One process per case would be more faithful to a real
    editing session and far slower; `--verbose` twice over is how to check it is stable.
    """
    original = path.read_text().splitlines()
    lines = list(original)
    cursors: dict[int, tuple[int, int]] = {}

    # Located against the untouched text, then mutated — otherwise one case's truncation
    # hides the anchor the next one is looking for.
    located = [(index, case, case.locate(original)) for index, case in enumerate(cases)]

    # Two cases may share a line only if at most one of them rewrites it. An inserted body
    # goes *after* its anchor, so it sits happily beside a truncation on that same line.
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

    # Insertions last, shallowest first, carrying a running offset — and shifting every cursor
    # already recorded at or below each insertion point. Two inserted bodies in one file
    # otherwise leave the deeper one's cursor pointing at whatever the shallower one pushed
    # down, and the probe then asks the wrong position and reports every name in scope. That
    # read exactly like a regression in the macro; hence `verify` below.
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

    An overlay is line surgery, and a cursor that lands a few lines off still returns a long,
    plausible completion list — every name in scope, none of them the one expected. That is
    indistinguishable from the feature being broken, so it is checked rather than assumed.
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
    """The text taking the item would actually write.

    Matched on instead of `label`, because a generated `const` is labelled with its whole
    value — `ScalarF32Ch = Chan { id: 268455954, .. }` — and the label is the one part of
    an item that exists for a human rather than for the buffer.
    """
    edit = item.get("textEdit") or {}
    return edit.get("newText") or item.get("insertText") or item.get("label", "")


def ranked(items: list[dict]) -> list[dict]:
    """The list in the order an editor would show it."""
    return sorted(items, key=lambda item: (item.get("sortText") or item.get("label", "")))


def measure(items: list[dict], case: Case) -> dict:
    """What the list says about `case.expect`.

    `bare_rank` is the DSL working as intended: the name on its own. `qualified_rank` is
    the same constant reachable only as `Ref::Choice::RED`, which is what this tree still
    does for a command argument — the per-statement glob scope that fixes it is not here.
    Recording both means the probe reports which one changed.
    """
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
        # Drops to 0 when the cursor maps to nothing, which is the failure the byte-offset
        # invariant guards against.
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

        # Both controls have to come back, or nothing else measured means anything.
        for case in cases[:2]:
            if measured[case.why]["bare_rank"] is None:
                print(
                    f"completion_probe: INCONCLUSIVE — the control {case.why!r} did not\n"
                    "  offer its own name, so the server is not answering and any other\n"
                    "  failure here would say nothing about the expansions.",
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
            "\n`offered` going false is the one that matters most: rust-analyzer found\n"
            "nothing to complete at the cursor, which is what happens when the two\n"
            "expansions stop agreeing up to it. Check that nothing was inserted ahead of\n"
            "the cursor, and that a marker-bearing name is still emitted carrying its\n"
            "marker — a mapped token whose text lost the marker is discarded.\n"
            "\n`bare` going false means the name is only reachable by its full path again.\n"
            "`rank` climbing means something else in scope now sorts above it.\n"
            "\nIf the change is deliberate, re-bless with --bless and say so in the commit.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
