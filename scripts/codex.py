#!/usr/bin/env python3
"""Debug proxy: companion stdin/stdout/stderr -> real codex CLI with IO logging."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import traceback
from datetime import datetime
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
_LOG_LOCK = threading.Lock()
_CHUNK = 16 * 1024
_FWD_CHUNK = 8 * 1024
_PROMPT_EOF_MARKER = b"</cqds_cli_sandwich>"


def _ts() -> str:
    return datetime.now().strftime("%Y-%m-%d %H:%M:%S")


def _project_root() -> Path:
    raw = str(os.environ.get("PROJECT_ROOT") or "").strip()
    if raw:
        return Path(raw).resolve()
    return REPO.resolve()


def _default_log_path() -> Path:
    override = str(os.environ.get("CODEX_PROXY_LOG") or "").strip()
    if override:
        return Path(override).resolve()
    return _project_root() / "logs" / "codex_proxy.log"


def _log_line(log_path: Path, line: str) -> None:
    text = str(line).rstrip("\n")
    with _LOG_LOCK:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        with log_path.open("a", encoding="utf-8", newline="\n") as fh:
            fh.write(text + "\n")
            fh.flush()


def _log_payload(log_path: Path, tag: str, data: bytes, *, extra: str = "") -> None:
    nbytes = len(data)
    suffix = f" {extra}".rstrip()
    _log_line(log_path, f"[{_ts()}]. #{tag} bytes={nbytes}{suffix}")
    if nbytes:
        try:
            body = data.decode("utf-8", errors="replace")
        except Exception:
            body = repr(data)
        for chunk_line in body.splitlines() or [""]:
            _log_line(log_path, chunk_line)


def _forward_write(out_fd, data: bytes) -> bool:
    if out_fd is None:
        return False
    try:
        offset = 0
        while offset < len(data):
            part = data[offset : offset + _FWD_CHUNK]
            written = out_fd.write(part)
            if not written:
                return False
            offset += int(written)
        out_fd.flush()
        return True
    except OSError:
        return False


def _pump_out(
    stream,
    out_fd,
    log_path: Path,
    tag: str,
    totals: dict[str, int],
    done: threading.Event,
    forward: list[bool],
) -> None:
    try:
        while True:
            chunk = stream.read(_CHUNK)
            if not chunk:
                break
            totals[tag] = int(totals.get(tag, 0)) + len(chunk)
            _log_payload(log_path, tag, chunk, extra=f"total={totals[tag]}")
            if forward[0]:
                if not _forward_write(out_fd, chunk):
                    forward[0] = False
                    _log_line(
                        log_path,
                        f"[{_ts()}]. #{tag} forward_disabled reason=parent_pipe_closed_or_full",
                    )
    except Exception as exc:
        _log_line(log_path, f"[{_ts()}]. #{tag} pump_error error={exc}")
    finally:
        done.set()
        try:
            stream.close()
        except Exception:
            pass


def _push_child_eof(child_stdin, log_path: Path, *, reason: str) -> None:
    try:
        child_stdin.flush()
    except Exception:
        pass
    try:
        child_stdin.close()
        _log_line(log_path, f"[{_ts()}]. #TX child_eof_pushed reason={reason}")
    except Exception as exc:
        _log_line(log_path, f"[{_ts()}]. #TX child_eof_error reason={reason} error={exc}")


def _prompt_eof_marker() -> bytes:
    custom = str(os.environ.get("CODEX_PROXY_PROMPT_EOF_MARKER") or "").strip().encode("utf-8")
    return custom or _PROMPT_EOF_MARKER


def _stdin_timeout_sec() -> float:
    raw = str(os.environ.get("CODEX_PROXY_STDIN_TIMEOUT_SEC") or "").strip()
    if not raw:
        return 10.0
    try:
        value = float(raw)
    except ValueError:
        return 10.0
    return value if value >= 0 else 10.0


def _emit_companion_fail(log_path: Path, reason: str, message: str, **extra) -> None:
    payload = {"reason": reason, "message": message}
    payload.update(extra)
    body = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
    print(f'<cqds_ticket_fail content="JSON">{body}</cqds_ticket_fail>', flush=True)
    _log_line(log_path, f"[{_ts()}]. #PROXY fail_inline_emitted tag=cqds_ticket_fail reason={reason}")


def _accept_stdin_eof_as_prompt() -> bool:
    raw = str(os.environ.get("CODEX_PROXY_ACCEPT_STDIN_EOF") or "").strip().lower()
    return raw in {"1", "true", "yes", "on"}


def _win_stdin_peek_bytes() -> int:
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.windll.kernel32
    handle = kernel32.GetStdHandle(-10)  # STD_INPUT_HANDLE
    if handle in (0, ctypes.c_void_p(-1).value):
        return 0
    avail = wintypes.DWORD(0)
    ok = kernel32.PeekNamedPipe(
        handle,
        None,
        0,
        None,
        ctypes.byref(avail),
        None,
    )
    if not ok:
        # ERROR_BROKEN_PIPE (109) => stdin write end closed
        if kernel32.GetLastError() == 109:
            return -1
        return 0
    return int(avail.value)


def _stdin_poll_available() -> int:
    """Bytes ready on stdin without blocking (0 = none). Main thread only on Windows."""
    try:
        if os.name == "nt":
            if sys.stdin.isatty():
                import msvcrt

                return 1 if msvcrt.kbhit() else 0
            return _win_stdin_peek_bytes()
        import select

        ready, _, _ = select.select([sys.stdin.buffer], [], [], 0)
        return _CHUNK if ready else 0
    except Exception:
        return 0


def _wait_prompt_stdin(
    log_path: Path,
    *,
    started_at: float,
    timeout_sec: float,
    marker: bytes,
    totals: dict[str, int],
) -> tuple[int, bytes]:
    """Read companion prompt on the main thread until marker, EOF, or timeout."""
    buf = bytearray()
    accept_eof = _accept_stdin_eof_as_prompt()
    is_tty = sys.stdin.isatty()
    _log_line(
        log_path,
        (
            f"[{_ts()}]. #PROXY waiting_for_companion_stdin timeout_sec={timeout_sec:g} "
            f"isatty={str(is_tty).lower()} accept_stdin_eof={str(accept_eof).lower()}"
        ),
    )
    if is_tty:
        _log_line(
            log_path,
            (
                f"[{_ts()}]. #PROXY tty_hint console input is line-buffered: "
                f"press Enter before Ctrl-Z; for manual tests set CODEX_PROXY_ACCEPT_STDIN_EOF=1"
            ),
        )

    while True:
        now = time.monotonic()
        elapsed = now - started_at
        if elapsed >= timeout_sec:
            _log_line(
                log_path,
                f"[{_ts()}]. #PROXY abort reason=stdin_prompt_timeout elapsed_sec={elapsed:.3f} bytes={len(buf)}",
            )
            _emit_companion_fail(
                log_path,
                "mcp_stdin_timeout",
                "MCP interaction error: companion did not deliver stdin prompt with EOF within timeout",
                diagnostic="codex_proxy_stdin_timeout",
                timeout_sec=timeout_sec,
                bytes_received=len(buf),
                stdin_isatty=is_tty,
            )
            return 3, b""

        pending = _stdin_poll_available()
        if pending < 0:
            chunk = b""
        elif pending > 0:
            if is_tty:
                chunk = sys.stdin.buffer.read1(_CHUNK)
            else:
                chunk = sys.stdin.buffer.read(min(_CHUNK, pending))
        else:
            chunk = None

        if chunk is not None:
            if chunk:
                buf.extend(chunk)
                totals["TX"] = len(buf)
                _log_payload(log_path, "TX", chunk, extra=f"total={totals['TX']} pre_spawn=true")
                if marker in buf:
                    return 0, bytes(buf)
                continue
            # EOF
            if not buf:
                _log_line(
                    log_path,
                    f"[{_ts()}]. #PROXY abort reason=empty_companion_stdin elapsed_sec={elapsed:.3f}",
                )
                _emit_companion_fail(
                    log_path,
                    "mcp_stdin_timeout",
                    "MCP interaction error: companion stdin closed before any prompt bytes",
                    diagnostic="codex_proxy_stdin_timeout",
                    timeout_sec=timeout_sec,
                    detail="empty_companion_stdin",
                    stdin_isatty=is_tty,
                )
                return 3, b""
            if accept_eof:
                _log_line(
                    log_path,
                    f"[{_ts()}]. #PROXY stdin_eof_accepted bytes={len(buf)} elapsed_sec={elapsed:.3f}",
                )
                return 0, bytes(buf)
            _log_line(
                log_path,
                (
                    f"[{_ts()}]. #PROXY abort reason=stdin_eof_before_prompt_marker "
                    f"elapsed_sec={elapsed:.3f} bytes={len(buf)}"
                ),
            )
            _emit_companion_fail(
                log_path,
                "mcp_stdin_timeout",
                "MCP interaction error: stdin EOF before prompt marker",
                diagnostic="codex_proxy_stdin_eof_before_marker",
                timeout_sec=timeout_sec,
                bytes_received=len(buf),
                stdin_isatty=is_tty,
                hint="companion prompt must include </cqds_cli_sandwich> or set CODEX_PROXY_ACCEPT_STDIN_EOF=1 for manual tests",
            )
            return 3, b""

        # Blocking read when poll is inconclusive (e.g. pipe without peek on some platforms).
        if not is_tty and os.name != "nt":
            import select

            remaining = max(0.0, timeout_sec - elapsed)
            ready, _, _ = select.select([sys.stdin.buffer], [], [], min(0.1, remaining))
            if ready:
                chunk = sys.stdin.buffer.read(_CHUNK)
                if not chunk:
                    continue
                buf.extend(chunk)
                totals["TX"] = len(buf)
                _log_payload(log_path, "TX", chunk, extra=f"total={totals['TX']} pre_spawn=true")
                if marker in buf:
                    return 0, bytes(buf)
            continue

        time.sleep(0.05)


def _spawn_child(log_path: Path, codex_argv: list[str]) -> subprocess.Popen:
    """Spawn codex; on Windows use shell for .cmd/.bat shims (npm/fnm)."""
    env = os.environ.copy()
    argv = list(codex_argv)
    prog = str(argv[0] or "").strip()
    resolved = shutil.which(prog, path=env.get("PATH"))
    spawn_kw: dict = {
        "stdin": subprocess.PIPE,
        "stdout": subprocess.PIPE,
        "stderr": subprocess.PIPE,
        "env": env,
        "close_fds": True,
    }
    use_shell = False
    spawn_target: list[str] | str = argv

    if resolved:
        argv[0] = resolved
        spawn_target = argv

    if os.name == "nt":
        low = str(argv[0]).lower()
        if low.endswith((".cmd", ".bat")) or not resolved:
            use_shell = True
            spawn_target = subprocess.list2cmdline(argv if resolved else codex_argv)
    elif not resolved:
        msg = f"executable not found in PATH: {prog}"
        _log_line(log_path, f"[{_ts()}]. #PROXY spawn_error error={msg}")
        raise FileNotFoundError(msg)

    _log_line(
        log_path,
        (
            f"[{_ts()}]. #PROXY spawn resolved={resolved or 'none'} "
            f"shell={str(use_shell).lower()} target={spawn_target!r}"
        ),
    )
    if use_shell:
        return subprocess.Popen(spawn_target, shell=True, **spawn_kw)  # noqa: S603
    return subprocess.Popen(spawn_target, **spawn_kw)  # noqa: S603


def _write_child(child_stdin, data: bytes) -> None:
    if not data:
        return
    child_stdin.write(data)
    child_stdin.flush()


def _pump_in(child_stdin, log_path: Path, totals: dict[str, int], *, initial: bytes = b"") -> int:
    """Read companion stdin on main thread; push EOF to codex child only."""
    marker = _prompt_eof_marker()
    total = int(totals.get("TX", 0))
    pending = bytearray(initial)
    child_open = True
    _log_line(
        log_path,
        (
            f"[{_ts()}]. #TX reader_start fileno={sys.stdin.fileno()} "
            f"isatty={str(sys.stdin.isatty()).lower()} initial_bytes={len(initial)}"
        ),
    )

    def _maybe_push_prompt_eof() -> None:
        nonlocal child_open
        if not child_open or marker not in pending:
            return
        end = pending.index(marker) + len(marker)
        _write_child(child_stdin, bytes(pending[:end]))
        del pending[:end]
        _push_child_eof(child_stdin, log_path, reason="prompt_marker")
        child_open = False
        if pending:
            _log_line(log_path, f"[{_ts()}]. #TX post_prompt_buffered bytes={len(pending)}")

    try:
        if pending:
            _maybe_push_prompt_eof()
            if child_open and len(pending) >= _CHUNK:
                _write_child(child_stdin, bytes(pending))
                pending.clear()

        while True:
            chunk = sys.stdin.buffer.read(_CHUNK)
            if not chunk:
                break
            total += len(chunk)
            totals["TX"] = total
            _log_payload(log_path, "TX", chunk, extra=f"total={total}")
            if not child_open:
                _log_line(log_path, f"[{_ts()}]. #TX post_prompt_ignored bytes={len(chunk)}")
                continue
            pending.extend(chunk)
            _maybe_push_prompt_eof()
            if child_open and len(pending) >= _CHUNK:
                _write_child(child_stdin, bytes(pending))
                pending.clear()
    except Exception as exc:
        _log_line(log_path, f"[{_ts()}]. #TX pump_error error={exc}")
    finally:
        _log_line(log_path, f"[{_ts()}]. #TX eof companion_stdin_closed bytes_total={total}")
        if child_open:
            if pending:
                _write_child(child_stdin, bytes(pending))
            _push_child_eof(child_stdin, log_path, reason="companion_stdin_eof")
    return total


def _parse_argv(argv: list[str]) -> tuple[Path, list[str]]:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--log-file", dest="log_file", default=None)
    parser.add_argument("--help", "-h", action="store_true")
    args, rest = parser.parse_known_args(argv)
    if args.help:
        print(
            "usage: codex.py [--log-file PATH] -- <codex argv...>\n"
            "       codex.py [--log-file PATH] codex <codex args...>\n"
            f"default log: {_default_log_path()}",
            file=sys.stderr,
        )
        raise SystemExit(0)
    log_path = Path(args.log_file).resolve() if args.log_file else _default_log_path()
    codex_argv = list(rest)
    if codex_argv and codex_argv[0] == "--":
        codex_argv = codex_argv[1:]
    if not codex_argv:
        print("codex.py: missing codex command argv", file=sys.stderr)
        raise SystemExit(2)
    return log_path, codex_argv


def main(argv: list[str] | None = None) -> int:
    log_path, codex_argv = _parse_argv(list(argv if argv is not None else sys.argv[1:]))
    totals: dict[str, int] = {"TX": 0, "RX": 0, "RXE": 0}
    cmd_text = subprocess.list2cmdline(codex_argv) if os.name == "nt" else " ".join(codex_argv)
    started_at = time.monotonic()
    marker = _prompt_eof_marker()
    stdin_timeout = _stdin_timeout_sec()
    _log_line(
        log_path,
        f"[{_ts()}]. #PROXY start proxy_pid={os.getpid()} cwd={os.getcwd()} cmd={cmd_text}",
    )

    rc = 1
    child: subprocess.Popen | None = None
    try:
        wait_rc, first = _wait_prompt_stdin(
            log_path,
            started_at=started_at,
            timeout_sec=stdin_timeout,
            marker=marker,
            totals=totals,
        )
        if wait_rc != 0:
            rc = wait_rc
            return rc

        child = _spawn_child(log_path, codex_argv)
        _log_line(log_path, f"[{_ts()}]. #PROXY child_pid={child.pid}")

        out_done = threading.Event()
        err_done = threading.Event()
        rx_forward = [True]
        rxe_forward = [True]
        pumps = [
            threading.Thread(
                target=_pump_out,
                args=(child.stdout, sys.stdout.buffer, log_path, "RX", totals, out_done, rx_forward),
                daemon=True,
            ),
            threading.Thread(
                target=_pump_out,
                args=(child.stderr, sys.stderr.buffer, log_path, "RXE", totals, err_done, rxe_forward),
                daemon=True,
            ),
        ]
        for th in pumps:
            th.start()

        _pump_in(child.stdin, log_path, totals, initial=first)
        rc = int(child.wait())
        out_done.wait(timeout=30)
        err_done.wait(timeout=30)
        for th in pumps:
            th.join(timeout=1)
    except Exception as exc:
        tb_text = traceback.format_exc()
        _log_line(
            log_path,
            f"[{_ts()}]. #PROXY fatal type={type(exc).__name__} error={exc}\n{tb_text}",
        )
        print(tb_text, file=sys.stderr, end="")
        return 1
    finally:
        _log_line(
            log_path,
            (
                f"[{_ts()}]. #PROXY exit rc={rc} "
                f"tx_bytes={totals.get('TX', 0)} rx_bytes={totals.get('RX', 0)} "
                f"rxe_bytes={totals.get('RXE', 0)}"
            ),
        )
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
