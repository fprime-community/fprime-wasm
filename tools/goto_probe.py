#!/usr/bin/env python3
"""Check that goto-definition still works through the `fprime_macros` expansions.

Span damage in a proc macro is invisible to `cargo build` and to the `.wasm` it produces, so
this asks rust-analyzer to resolve tokens the author wrote inside a macro-expanded body. A
token the macro deleted or re-spanned on `Span::call_site()` resolves to nothing.

The `sequence = ".."` probes name the file they must open: the string only resolves through
the `include_str!` path literal `#[fprime_test]` gives its span.

A plain-Rust control has to resolve first, or the run is inconclusive: an unready server
answers everything with nothing. `cargo check` runs first so the proc macro is current.

Usage:
    python3 tools/goto_probe.py            # all probes
    python3 tools/goto_probe.py --verbose  # and the target each one resolved to
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import threading
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# The rust-analyzer the editor actually uses. `~/.cargo/bin/rust-analyzer` is a rustup proxy
# that may not resolve to a real server.
SERVER_GLOBS = [
    Path.home() / ".vscode/extensions",
    Path.home() / ".vscode-server/extensions",
    Path.home() / ".vscode-insiders/extensions",
]

# How long to wait for the project to load before giving up.
INDEX_TIMEOUT_S = 300


def find_server() -> Path:
    for root in SERVER_GLOBS:
        if not root.is_dir():
            continue
        for extension in sorted(root.glob("rust-lang.rust-analyzer-*")):
            binary = extension / "server" / "rust-analyzer"
            if binary.is_file() and os.access(binary, os.X_OK):
                return binary
    print(
        "goto_probe: no rust-analyzer found under "
        + ", ".join(str(root) for root in SERVER_GLOBS)
        + "\n  Install the VS Code extension; the rustup proxy at ~/.cargo/bin is not a server.",
        file=sys.stderr,
    )
    sys.exit(2)


class Client:
    """Barely enough LSP to ask where a token is defined."""

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
        header = f"Content-Length: {len(body)}\r\n\r\n".encode()
        assert self.process.stdin is not None
        self.process.stdin.write(header + body)
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
        self._send(
            {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}
        )
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
                        "definition": {"linkSupport": True},
                    },
                    "experimental": {"serverStatusNotification": True},
                },
                "initializationOptions": {
                    # The sequences are a Wasm target; without this the bins do not load and
                    # every probe inside one is inconclusive.
                    "cargo": {"allTargets": True},
                    "procMacro": {"enable": True},
                    "checkOnSave": {"enable": False},
                },
            },
        )
        self.notify("initialized", {})

    def open(self, path: Path) -> None:
        self.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": path.as_uri(),
                    "languageId": "rust",
                    "version": 1,
                    "text": path.read_text(),
                }
            },
        )

    def definition(self, path: Path, line: int, character: int) -> list:
        reply = self.request(
            "textDocument/definition",
            {
                "textDocument": {"uri": path.as_uri()},
                "position": {"line": line, "character": character},
            },
        )
        result = reply.get("result") or []
        return result if isinstance(result, list) else [result]

    def shutdown(self) -> None:
        try:
            self.request("shutdown", {}, timeout=10)
            self.notify("exit", {})
        except Exception:
            pass
        self.process.terminate()


class Probe:
    """One token to resolve, located by searching rather than by a pinned line number.

    `expect` names the file the token has to resolve *to*, for the probes where landing
    somewhere is not the same as landing in the right place.
    """

    def __init__(
        self, path: str, needle: str, token: str, why: str, expect: str | None = None
    ) -> None:
        self.path = REPO / path
        self.needle = needle
        self.token = token
        self.why = why
        self.expect = expect

    def position(self) -> tuple[int, int]:
        """The line and column of `token` inside the line matching `needle`."""
        for number, line in enumerate(self.path.read_text().splitlines()):
            if self.needle in line:
                column = line.index(self.token, line.index(self.needle))
                # Aim at the middle of the token, so an off-by-one at a boundary cannot make
                # a failing probe look like it is asking about the punctuation next door.
                return number, column + len(self.token) // 2
        raise LookupError(f"{self.path}: no line containing {self.needle!r}")


SEQUENCE = "crates/example/src/bin/example.rs"
TESTS = "crates/example/tests/behavior.rs"
LONG = "crates/example/tests/long_running.rs"
LIMITS = "crates/example/tests/limits.rs"

PROBES = [
    # --- the control. Plain Rust, no macro involved. If this does not resolve, the server is
    # not ready and every other answer is meaningless.
    Probe(SEQUENCE, "set_fail_mode(FailMode::Permissive)", "set_fail_mode", "control"),
    # --- inside `#[fprime_main]`: the existing DSL.
    Probe(SEQUENCE, "CdhCore.cmdDisp.CMD_NO_OP()", "CdhCore", "sequence: receiver"),
    Probe(SEQUENCE, "CdhCore.cmdDisp.CMD_NO_OP()", "cmdDisp", "sequence: component"),
    Probe(SEQUENCE, "CdhCore.cmdDisp.CMD_NO_OP()", "CMD_NO_OP", "sequence: command"),
    Probe(SEQUENCE, 'CMD_NO_OP_STRING("STRINGS")', "CMD_NO_OP_STRING", "sequence: string command"),
    Probe(SEQUENCE, "CdhCore.events.EventsDropped()", "EventsDropped", "sequence: telemetry"),
    Probe(SEQUENCE, "Ref.dpDemo.SelectColor(BLUE)", "SelectColor", "sequence: command"),
    Probe(SEQUENCE, "Ref.dpDemo.SelectColor(BLUE)", "BLUE", "sequence: enum constant"),
    Probe(SEQUENCE, "GLUTTON_OF_CHOICE(ChoiceSlurry", "GLUTTON_OF_CHOICE", "sequence: struct arg"),
    # --- inside `#[fprime_test]`: the descriptor pass. A bare descriptor is the case with no
    # `if false { .. }` copy of the author's tokens, so it is the one span-copying has to carry.
    Probe(TESTS, "t.expect_command(CdhCore.cmdDisp.CMD_NO_OP())", "CMD_NO_OP", "test: command call"),
    Probe(TESTS, "t.never_command(Ref.wasmSeq.LOAD)", "Ref", "test: bare receiver"),
    Probe(TESTS, "t.never_command(Ref.wasmSeq.LOAD)", "wasmSeq", "test: bare component"),
    Probe(TESTS, "t.never_command(Ref.wasmSeq.LOAD)", "LOAD", "test: bare command"),
    Probe(TESTS, "t.initial_telemetry(CdhCore.events.EventsDropped, 5)", "EventsDropped", "test: channel"),
    Probe(TESTS, "t.expect_command(Ref.wasmSeq.LOAD(", "LOAD", "test: command with arguments"),
    Probe(LONG, "t.initial_parameter(Ref.wasmSeq.INSTRUCTION_FUEL, 20)", "INSTRUCTION_FUEL", "test: parameter"),
    Probe(LONG, "t.expect_telemetry_read(CdhCore.events.EventsDropped)", "EventsDropped", "test: bare channel"),
    # --- the `sequence = ".."` source link. The name is a string, not a path, so it only
    # resolves because `#[fprime_test]` emits an `include_str!` whose path literal carries the
    # string's span. `expect` matters here: the link is only useful if it opens *that* sequence.
    Probe(TESTS, '#[fprime_test(sequence = "no_op")]', "no_op", "test: sequence name", "no_op.rs"),
    Probe(TESTS, '#[fprime_test(sequence = "example")]', "example", "test: sequence name", "example.rs"),
    Probe(LONG, '#[fprime_test(sequence = "worker")]', "worker", "test: sequence name", "worker.rs"),
    Probe(LONG, '#[fprime_test(sequence = "poller")]', "poller", "test: sequence name", "poller.rs"),
    # --- the `limits = ".."` link, the same trick pointed at a `sequencer.toml`. Nothing about
    # this string is Rust either, and here the file it opens is the one the author typed.
    Probe(
        LIMITS,
        'limits = "sequencer-short-events.toml"',
        "sequencer-short-events.toml",
        "test: limits file",
        "sequencer-short-events.toml",
    ),
]


def targets_of(locations: list) -> list[str]:
    """Every distinct `file:line` the token resolved to, in order."""
    targets = []
    for location in locations:
        uri = location.get("targetUri") or location.get("uri", "")
        span = location.get("targetSelectionRange") or location.get("range") or {}
        line = span.get("start", {}).get("line", -1)
        target = f"{re.sub(r'.*/', '', uri)}:{line + 1}"
        if target not in targets:
            targets.append(target)
    return targets


def target_of(locations: list) -> str:
    """The targets as one field: `-` for none, `a; b` when a token has more than one."""
    return "; ".join(targets_of(locations)) or "-"


def main() -> int:
    verbose = "--verbose" in sys.argv

    # A stale proc-macro dylib answers for the macro as it was, not as it is.
    print("goto_probe: cargo check (so the proc macro is current)", file=sys.stderr)
    for args in (
        ["cargo", "check", "-p", "example", "--target", "wasm32v1-none", "--features", "wasm"],
        ["cargo", "check", "-p", "example", "--tests"],
    ):
        result = subprocess.run(args, cwd=REPO, capture_output=True)
        if result.returncode != 0:
            print(result.stderr.decode()[-3000:], file=sys.stderr)
            print(f"goto_probe: `{' '.join(args)}` failed", file=sys.stderr)
            return 2

    client = Client(find_server())
    try:
        client.initialize()
        for path in {probe.path for probe in PROBES}:
            client.open(path)

        print("goto_probe: waiting for the project to load", file=sys.stderr)
        if not client.quiescent.wait(INDEX_TIMEOUT_S):
            print("goto_probe: rust-analyzer never went quiescent", file=sys.stderr)
            return 2

        results = []
        for probe in PROBES:
            line, character = probe.position()
            locations = client.definition(probe.path, line, character)
            results.append((probe, locations))

        control, control_locations = results[0]
        if not control_locations:
            print(
                "goto_probe: INCONCLUSIVE — the plain-Rust control did not resolve, so the\n"
                "  server is not answering and a failure here would say nothing about spans.",
                file=sys.stderr,
            )
            return 2

        resolved = 0
        for probe, locations in results:
            targets = targets_of(locations)
            target = target_of(locations)
            # Every token here names exactly one thing, so two answers is a failure and not a
            # richer success: it means invented tokens are claiming an author range as well.
            ok = len(targets) == 1 and (
                probe.expect is None or targets[0].startswith(f"{probe.expect}:")
            )
            resolved += ok
            mark = "ok  " if ok else "FAIL"
            detail = f"  -> {target}" if verbose or not ok else ""
            print(f"  {mark} {probe.why:<32} {probe.token}{detail}")

        total = len(results)
        print(f"\ngoto_probe: {resolved}/{total} resolved")
        if resolved < total:
            print(
                "\nA token that resolves to nothing was deleted from the expansion or re-minted\n"
                "on Span::call_site(). Interpolate the tokens the author wrote.\n"
                "\nA token that resolves to more than one place is the inverse: a token the\n"
                "expansion invented is claiming that author range too. Tokens you invent may\n"
                "borrow an author *location* only where nothing is goto-able — `spanned_desc_path`\n"
                "uses the chain's `.` for this, and an ident would fail here.\n"
                "\nA sequence name that resolves to the wrong file, or to nothing while its\n"
                "`src/bin/<name>.rs` exists, means `fprime_test::source_link` stopped putting the\n"
                "whole absolute path in the one literal that carries the author's span. Composing\n"
                "it — `concat!(env!(\"CARGO_MANIFEST_DIR\"), ..)` — compiles and kills the link.",
                file=sys.stderr,
            )
            return 1
        return 0
    finally:
        client.shutdown()


if __name__ == "__main__":
    sys.exit(main())
