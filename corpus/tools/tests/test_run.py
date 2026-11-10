"""Tests of the mesher runs, with small shell scripts standing in for the mesher."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import reference  # noqa: E402
import run  # noqa: E402

SUMMARY = "echo 'ipcorpus: 7 calls recorded, 5 stored, 9 points; {cascades} cascade entries' >&2"
WRITE_TET = 'echo tet > "$1.tet"'
WRITE_CORPUS = 'echo corpus > "$IPCORPUS_OUT"'


class RunTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.tmp = Path(self._tmp.name)
        self.model = self.tmp / "7.off"
        self.model.write_text("OFF\n")
        self.runs = self.tmp / "runs"
        self.out = self.tmp / "corpus" / "7.ipcorpus"

    def mesher(self, *lines):
        path = self.tmp / "cdt"
        path.write_text("#!/bin/sh\n" + "\n".join(lines) + "\n")
        path.chmod(0o755)
        return path

    def original(self, *lines, timeout=10):
        return run.run_original(7, self.model, timeout, self.mesher(*lines), self.runs)

    def recording(self, *lines):
        return run.run_recording(7, self.model, 10, self.out, None, self.mesher(*lines), self.runs)

    def test_original_is_valid_with_status_zero_and_a_tet_output(self):
        outcome = self.original(WRITE_TET)
        self.assertEqual((outcome.status, outcome.reason), ("ok", ""))
        self.assertFalse((self.runs / "original" / "7").exists())

    def test_status_zero_without_a_tet_output_is_a_failure(self):
        outcome = self.original("echo 'Symbolic perturbation failed!' >&2")
        self.assertEqual((outcome.status, outcome.reason), ("failed", "no .tet output"))
        self.assertTrue((self.runs / "original" / "7" / "stderr.txt").exists())

    def test_an_empty_tet_output_is_a_failure(self):
        self.assertEqual(self.original(': > "$1.tet"').status, "failed")

    def test_a_non_zero_status_is_a_failure(self):
        outcome = self.original(WRITE_TET, "exit 3")
        self.assertEqual((outcome.status, outcome.reason), ("failed", "exit status 3"))

    def test_a_crash_is_reported_with_its_signal(self):
        outcome = self.original(WRITE_TET, "kill -SEGV $$")
        self.assertEqual((outcome.status, outcome.reason), ("failed", "signal SIGSEGV"))

    def test_the_mesher_runs_without_core_dumps(self):
        outcome = self.original('[ "$(ulimit -c)" = 0 ] && ' + WRITE_TET)
        self.assertEqual(outcome.status, "ok")

    def test_a_run_over_the_time_limit_is_killed(self):
        outcome = self.original("sleep 30", WRITE_TET, timeout=0.5)
        self.assertEqual(outcome.status, "timeout")
        self.assertLess(outcome.seconds, 10)

    def test_a_recording_run_moves_its_corpus_and_reads_the_summary(self):
        outcome = self.recording(WRITE_TET, WRITE_CORPUS, SUMMARY.format(cascades=7))
        self.assertEqual((outcome.status, outcome.calls, outcome.stored), ("ok", 7, 5))
        self.assertEqual(self.out.read_text(), "corpus\n")

    def test_the_error_routine_fails_the_recording_run_despite_status_zero(self):
        outcome = self.recording(
            WRITE_TET, "echo 'ipcorpus: recording abandoned after 3 calls' >&2")
        self.assertEqual((outcome.status, outcome.reason), ("failed", "abandoned"))
        self.assertFalse(self.out.exists())

    def test_a_recording_run_without_corpus_fails(self):
        outcome = self.recording(WRITE_TET, SUMMARY.format(cascades=7))
        self.assertEqual((outcome.status, outcome.reason), ("failed", "no corpus output"))

    def test_more_cascades_than_recorded_calls_stops_everything(self):
        with self.assertRaises(reference.ReferenceError):
            self.recording(WRITE_TET, WRITE_CORPUS, SUMMARY.format(cascades=8))

    def test_an_earlier_outcome_is_reused_only_when_it_decides_the_new_limit(self):
        ok = run.Outcome(1, "ok", 100.0)
        timeout = run.Outcome(1, "timeout", 60.0, reason="timeout")
        self.assertTrue(run.within(ok, 100) and not run.within(ok, 99))
        self.assertTrue(run.settled(ok, 50) and run.settled(timeout, 60))
        self.assertFalse(run.settled(timeout, 120) or run.settled(None, 10))

    def test_logs_round_trip(self):
        outcomes = {3: run.Outcome(3, "ok", 1.5, 10, 4, ""),
                    1: run.Outcome(1, "timeout", 600.0, 0, 0, "timeout")}
        path = self.tmp / "log.csv"
        run.write_log(path, outcomes)
        self.assertEqual(run.read_log(path), {1: run.Outcome(1, "timeout", 600.0, 0, 0, "timeout"),
                                              3: run.Outcome(3, "ok", 1.5, 10, 4, "")})


if __name__ == "__main__":
    unittest.main()
