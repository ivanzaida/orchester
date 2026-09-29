"""Observe host foreground PIDs while a separately launched verifier app runs.

This tool only reads the foreground application. Start ``watch`` on the host before
launching the app, provide its PID in a run-owned file, then stop after the app exits.
The result covers sampled instants, not every instant between samples.
"""

import argparse
import ctypes
import json
import subprocess
import sys
import time
from ctypes import wintypes
from datetime import datetime, timezone
from functools import lru_cache
from pathlib import Path


MAC_PID_SCRIPT = (
    'use framework "AppKit"\n'
    "set frontApp to current application's NSWorkspace's sharedWorkspace()'s frontmostApplication()\n"
    "if frontApp is missing value then return -1\n"
    "return (frontApp's processIdentifier()) as integer"
)


def utc_now():
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


@lru_cache(maxsize=1)
def windows_user32():
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    user32.GetForegroundWindow.restype = wintypes.HWND
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user32.GetWindowThreadProcessId.restype = wintypes.DWORD
    return user32


def foreground_pid():
    if sys.platform == "win32":
        user32 = windows_user32()
        window = user32.GetForegroundWindow()
        if not window:
            return None
        pid = wintypes.DWORD()
        if not user32.GetWindowThreadProcessId(window, ctypes.byref(pid)) or not pid.value:
            return None
        return pid.value
    if sys.platform == "darwin":
        result = subprocess.run(
            ["/usr/bin/osascript", "-l", "AppleScript", "-e", MAC_PID_SCRIPT],
            capture_output=True,
            text=True,
            timeout=5,
            check=True,
        )
        pid = int(result.stdout.strip())
        return pid if pid > 0 else None
    raise RuntimeError("foreground observation supports Windows and macOS only")


def write_record(stream, kind, **fields):
    record = {"kind": kind, "utc": utc_now(), "monotonic_ns": time.monotonic_ns(), **fields}
    stream.write(json.dumps(record, ensure_ascii=True) + "\n")
    stream.flush()


def read_app_pid(path):
    value = path.read_text(encoding="ascii").strip()
    if not value.isdecimal() or int(value) <= 0:
        raise ValueError("PID file must contain one positive decimal PID")
    return int(value)


def watch(timeline, pid_file, stop_file, interval_ms, max_seconds, sampler=foreground_pid):
    if sys.platform not in ("win32", "darwin") and sampler is foreground_pid:
        raise RuntimeError("foreground observation supports Windows and macOS only")
    if pid_file.exists() or stop_file.exists():
        raise ValueError("PID and stop files must not exist before watching")
    if not all(path.parent.is_dir() for path in (timeline, pid_file, stop_file)):
        raise ValueError("timeline, PID and stop-file parent directories must already exist")
    deadline = time.monotonic() + max_seconds
    with timeline.open("x", encoding="utf-8") as stream:
        write_record(stream, "start", interval_ms=interval_ms)
        reason = "timeout"
        own_pid = None
        try:
            while time.monotonic() < deadline:
                try:
                    if pid_file.exists():
                        observed = read_app_pid(pid_file)
                        if own_pid is None:
                            own_pid = observed
                            write_record(stream, "app", pid=own_pid)
                        elif observed != own_pid:
                            reason = "pid-file-changed"
                            break
                    elif own_pid is not None:
                        reason = "pid-file-removed"
                        break
                except (OSError, UnicodeError, ValueError):
                    reason = "pid-file-error"
                    break
                try:
                    pid = sampler()
                    error = None
                except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as failure:
                    pid = None
                    error = type(failure).__name__
                write_record(stream, "sample", pid=pid, error=error)
                if error:
                    reason = "sampler-error"
                    break
                if stop_file.exists():
                    reason = "stop-file" if own_pid is not None else "stop-before-app"
                    break
                time.sleep(interval_ms / 1000)
        except KeyboardInterrupt:
            reason = "interrupted"
        write_record(stream, "end", reason=reason)
    return reason == "stop-file"


def verdict(records):
    if not records or any(not isinstance(record, dict) for record in records):
        return "inconclusive", "incomplete timeline"
    if records[0].get("kind") != "start" or records[-1].get("kind") != "end":
        return "inconclusive", "incomplete timeline"
    middle = records[1:-1]
    if any(record.get("kind") not in ("app", "sample") for record in middle):
        return "inconclusive", "timeline has an invalid record"
    markers = [(index, record) for index, record in enumerate(middle) if record.get("kind") == "app"]
    if len(markers) != 1:
        return "inconclusive", "timeline has no unique app PID marker"
    marker_index, marker = markers[0]
    own_pid = marker.get("pid")
    if not isinstance(own_pid, int) or own_pid <= 0:
        return "inconclusive", "app PID marker is invalid"
    samples = [record for record in middle if record.get("kind") == "sample"]
    if any(sample.get("pid") == own_pid for sample in samples):
        return "fail", "verifier app PID was foreground"
    if not any(record.get("kind") == "sample" for record in middle[marker_index + 1 :]):
        return "inconclusive", "no foreground sample after app PID marker"
    if records[-1].get("reason") != "stop-file":
        return "inconclusive", "watcher did not reach the stop file"
    interval = records[0].get("interval_ms")
    if not isinstance(interval, int) or not 20 <= interval <= 1000:
        return "inconclusive", "invalid sampling interval"
    if any(
        sample.get("pid") is None
        or not isinstance(sample.get("pid"), int)
        or sample.get("pid") <= 0
        or sample.get("error")
        for sample in samples
    ):
        return "inconclusive", "foreground PID was unavailable"
    times = [record.get("monotonic_ns") for record in records]
    if any(not isinstance(value, int) for value in times):
        return "inconclusive", "timeline has invalid timestamps"
    max_gap_ns = max(100, interval * 3) * 1_000_000
    if any(later < earlier or later - earlier > max_gap_ns for earlier, later in zip(times, times[1:])):
        return "inconclusive", "timeline has a sampling gap"
    return "pass", "own PID was absent at the observed instants"


def judge(timeline):
    try:
        records = [json.loads(line) for line in timeline.read_text(encoding="utf-8").splitlines()]
        outcome, detail = verdict(records)
    except (OSError, UnicodeError, json.JSONDecodeError):
        outcome, detail = "inconclusive", "timeline could not be read"
    print(json.dumps({"verdict": outcome, "detail": detail}, ensure_ascii=True))
    return {"pass": 0, "fail": 1, "inconclusive": 2}[outcome]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    record = actions.add_parser("watch", help="sample foreground PIDs on the host")
    record.add_argument("--timeline", type=Path, required=True)
    record.add_argument("--pid-file", type=Path, required=True)
    record.add_argument("--stop-file", type=Path, required=True)
    record.add_argument("--interval-ms", type=int, default=200)
    record.add_argument("--max-seconds", type=int, default=1800)
    check = actions.add_parser("judge", help="compare the completed timeline with one app PID")
    check.add_argument("--timeline", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.action == "judge":
        return judge(args.timeline)
    if not 20 <= args.interval_ms <= 1000 or args.max_seconds <= 0:
        parser.error("interval must be 20-1000 ms and max seconds must be positive")
    try:
        return 0 if watch(args.timeline, args.pid_file, args.stop_file, args.interval_ms, args.max_seconds) else 2
    except (OSError, ValueError, RuntimeError) as error:
        print(f"foreground watcher unavailable ({type(error).__name__}): {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
