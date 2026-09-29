"""Synthetic verdict and recording tests for the host foreground watcher."""

import importlib.util
import io
import json
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "foreground-watch.py"
REPO_TMP = Path(__file__).resolve().parents[2] / ".tmp"
spec = importlib.util.spec_from_file_location("foreground_watch", SCRIPT)
watcher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(watcher)


def setUpModule():
    REPO_TMP.mkdir(parents=True, exist_ok=True)


def timeline(pids, *, marker_at=0, end="stop-file", interval=200, gap_ms=200, error=None):
    records = [{"kind": "start", "monotonic_ns": 0, "interval_ms": interval}]
    stamp = 0
    for index, pid in enumerate(pids):
        stamp += gap_ms * 1_000_000
        if marker_at == index:
            records.append({"kind": "app", "monotonic_ns": stamp, "pid": 333})
        records.append({"kind": "sample", "monotonic_ns": stamp, "pid": pid, "error": error})
    if marker_at == len(pids):
        records.append({"kind": "app", "monotonic_ns": stamp, "pid": 333})
    records.append({"kind": "end", "monotonic_ns": stamp + gap_ms * 1_000_000, "reason": end})
    return records


class VerdictTests(unittest.TestCase):
    def test_pass_covers_only_observed_instants(self):
        self.assertEqual(watcher.verdict(timeline([111, 222, 111]))[0], "pass")

    def test_own_pid_is_failure_even_if_other_samples_are_unavailable(self):
        self.assertEqual(watcher.verdict(timeline([None, 333, 111]))[0], "fail")
        self.assertEqual(watcher.verdict(timeline([333, 111], marker_at=1))[0], "fail")
        self.assertEqual(watcher.verdict(timeline([333], marker_at=1))[0], "fail")

    def test_null_or_error_is_inconclusive(self):
        self.assertEqual(watcher.verdict(timeline([111, None]))[0], "inconclusive")
        self.assertEqual(watcher.verdict(timeline([111], error="OSError"))[0], "inconclusive")

    def test_gap_and_timeout_are_inconclusive(self):
        self.assertEqual(watcher.verdict(timeline([111, 222], gap_ms=1200))[0], "inconclusive")
        self.assertEqual(watcher.verdict(timeline([111], end="timeout"))[0], "inconclusive")

    def test_missing_or_malformed_timeline_is_inconclusive(self):
        self.assertEqual(watcher.verdict([])[0], "inconclusive")
        self.assertEqual(watcher.verdict(timeline([]))[0], "inconclusive")
        broken = timeline([111])
        broken[1]["monotonic_ns"] = "unknown"
        self.assertEqual(watcher.verdict(broken)[0], "inconclusive")

    def test_marker_and_post_marker_sample_are_required(self):
        self.assertEqual(watcher.verdict(timeline([111], marker_at=None))[0], "inconclusive")
        self.assertEqual(watcher.verdict(timeline([111], marker_at=1))[0], "inconclusive")

    def test_judge_exit_codes(self):
        with tempfile.TemporaryDirectory(dir=REPO_TMP) as directory:
            path = Path(directory) / "timeline.jsonl"
            for pids, expected in [([111], 0), ([333], 1), ([None], 2)]:
                path.write_text("\n".join(json.dumps(item) for item in timeline(pids)) + "\n", encoding="utf-8")
                with redirect_stdout(io.StringIO()):
                    self.assertEqual(watcher.judge(path), expected)


class RecordTests(unittest.TestCase):
    def test_watch_starts_before_sampling_and_stops_on_file(self):
        with tempfile.TemporaryDirectory(dir=REPO_TMP) as directory:
            root = Path(directory)
            output = root / "timeline.jsonl"
            pid_file = root / "app.pid"
            stop_file = root / "stop"
            calls = 0

            def sample():
                nonlocal calls
                calls += 1
                if calls == 1:
                    pid_file.write_text("333", encoding="ascii")
                else:
                    stop_file.touch()
                return 111

            self.assertTrue(watcher.watch(output, pid_file, stop_file, 200, 1, sampler=sample))
            records = [json.loads(line) for line in output.read_text(encoding="utf-8").splitlines()]
            self.assertEqual([record["kind"] for record in records], ["start", "sample", "app", "sample", "end"])
            outcome, detail = watcher.verdict(records)
            self.assertEqual(outcome, "pass", detail)

    def test_sampler_error_is_recorded_as_inconclusive(self):
        with tempfile.TemporaryDirectory(dir=REPO_TMP) as directory:
            root = Path(directory)
            pid_file = root / "app.pid"
            calls = 0

            def fail():
                nonlocal calls
                calls += 1
                if calls == 1:
                    pid_file.write_text("333", encoding="ascii")
                    return 111
                raise OSError("synthetic failure")

            output = root / "timeline.jsonl"
            self.assertFalse(watcher.watch(output, pid_file, root / "stop", 200, 1, sampler=fail))
            records = [json.loads(line) for line in output.read_text(encoding="utf-8").splitlines()]
            self.assertEqual(records[-1]["reason"], "sampler-error")
            self.assertEqual(records[3]["error"], "OSError")
            self.assertEqual(watcher.verdict(records)[0], "inconclusive")

    def test_stop_before_pid_file_is_inconclusive(self):
        with tempfile.TemporaryDirectory(dir=REPO_TMP) as directory:
            root = Path(directory)
            stop_file = root / "stop"

            def sample():
                stop_file.touch()
                return 111

            output = root / "timeline.jsonl"
            self.assertFalse(watcher.watch(output, root / "app.pid", stop_file, 200, 1, sampler=sample))
            records = [json.loads(line) for line in output.read_text(encoding="utf-8").splitlines()]
            self.assertEqual(records[-1]["reason"], "stop-before-app")
            self.assertEqual(watcher.verdict(records)[0], "inconclusive")

    def test_changed_pid_file_stops_watcher(self):
        with tempfile.TemporaryDirectory(dir=REPO_TMP) as directory:
            root = Path(directory)
            pid_file = root / "app.pid"
            calls = 0

            def sample():
                nonlocal calls
                calls += 1
                pid_file.write_text("333" if calls == 1 else "444", encoding="ascii")
                return 111

            output = root / "timeline.jsonl"
            self.assertFalse(watcher.watch(output, pid_file, root / "stop", 200, 1, sampler=sample))
            records = [json.loads(line) for line in output.read_text(encoding="utf-8").splitlines()]
            self.assertEqual(records[-1]["reason"], "pid-file-changed")
            self.assertEqual(watcher.verdict(records)[0], "inconclusive")


if __name__ == "__main__":
    unittest.main()
