"""Tests of the mesher runs, with small shell scripts standing in for the mesher."""

import hashlib
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
# The digest of the output that WRITE_TET writes.
TET_DIGEST = hashlib.sha256(b"tet\n").hexdigest()


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

    def recording(self, *lines, expected=TET_DIGEST):
        return run.run_recording(7, self.model, 10, self.out, expected, None,
                                 self.mesher(*lines), self.runs)

    def test_original_is_valid_with_status_zero_and_a_tet_output(self):
        outcome = self.original(WRITE_TET)
        self.assertEqual((outcome.status, outcome.reason), ("ok", ""))
        self.assertFalse((self.runs / "original" / "7").exists())

    def test_the_original_run_keeps_the_digest_of_its_output(self):
        self.assertEqual(self.original(WRITE_TET).digest, TET_DIGEST)
        self.assertEqual(self.original("exit 3").digest, "")

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

    def test_a_recording_with_the_output_of_the_original_succeeds(self):
        outcome = self.recording(WRITE_TET, WRITE_CORPUS, SUMMARY.format(cascades=7))
        self.assertEqual((outcome.status, outcome.digest), ("ok", TET_DIGEST))

    def test_a_recording_whose_output_differs_from_the_original_fails(self):
        outcome = self.recording('echo other > "$1.tet"', WRITE_CORPUS,
                                 SUMMARY.format(cascades=7))
        self.assertEqual((outcome.status, outcome.reason), ("failed", "output differs"))
        self.assertNotEqual(outcome.digest, TET_DIGEST)
        self.assertFalse(self.out.exists())

    def test_a_recording_run_removes_an_earlier_corpus_first(self):
        self.out.parent.mkdir(parents=True)
        self.out.write_text("old\n")
        outcome = self.recording(WRITE_TET, "exit 3")
        self.assertEqual(outcome.status, "failed")
        self.assertFalse(self.out.exists())

    def test_an_original_run_without_digest_is_run_again(self):
        old = run.Outcome(1, "ok", 1.0)
        self.assertFalse(run.settled(old, 10))
        self.assertTrue(run.settled(run.Outcome(1, "ok", 1.0, digest=TET_DIGEST), 10))
        self.assertTrue(run.settled(run.Outcome(1, "failed", 1.0, reason="exit status 3"), 10))

    def test_a_recording_fails_its_model_unless_it_timed_out_under_a_shorter_limit(self):
        self.assertFalse(run.failed(None, 10))
        self.assertFalse(run.failed(run.Outcome(1, "ok", 1.0, 7, 5, digest=TET_DIGEST), 10))
        self.assertTrue(run.failed(run.Outcome(1, "failed", 1.0, reason="output differs"), 10))
        timeout = run.Outcome(1, "timeout", 60.0, reason="timeout")
        self.assertTrue(run.failed(timeout, 60))
        self.assertFalse(run.failed(timeout, 120))

    def test_a_recording_is_current_only_with_the_digest_of_the_original(self):
        original = run.Outcome(1, "ok", 1.0, digest=TET_DIGEST)
        self.assertTrue(run.current(run.Outcome(1, "ok", 1.0, 7, 5, digest=TET_DIGEST), original))
        self.assertFalse(run.current(run.Outcome(1, "ok", 1.0, 7, 5), original))
        self.assertFalse(run.current(None, original))
        self.assertFalse(run.current(run.Outcome(1, "ok", 1.0, 7, 5, digest="0" * 64),
                                     original))

    def test_reconcile_removes_the_corpus_files_of_other_models(self):
        directory = self.tmp / "corpus"
        directory.mkdir()
        for name in ["1.ipcorpus", "2.ipcorpus", "3.ipcorpus", "run.csv"]:
            (directory / name).write_text("x")
        self.assertEqual(run.reconcile(directory, {1, 3}), [2])
        self.assertEqual(sorted(p.name for p in directory.iterdir()),
                         ["1.ipcorpus", "3.ipcorpus", "run.csv"])

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
        ok = run.Outcome(1, "ok", 100.0, digest=TET_DIGEST)
        timeout = run.Outcome(1, "timeout", 60.0, reason="timeout")
        self.assertTrue(run.within(ok, 100) and not run.within(ok, 99))
        self.assertTrue(run.settled(ok, 50) and run.settled(timeout, 60))
        self.assertFalse(run.settled(timeout, 120) or run.settled(None, 10))

    def test_logs_round_trip(self):
        outcomes = {3: run.Outcome(3, "ok", 1.5, 10, 4, "", TET_DIGEST),
                    1: run.Outcome(1, "timeout", 600.0, 0, 0, "timeout")}
        path = self.tmp / "log.csv"
        run.write_log(path, outcomes)
        self.assertEqual(run.read_log(path), outcomes)

    def test_a_log_without_digests_reads_as_empty_digests(self):
        path = self.tmp / "log.csv"
        path.write_text("file_id,status,seconds,calls,stored,reason\n3,ok,1.50,10,4,\n")
        self.assertEqual(run.read_log(path), {3: run.Outcome(3, "ok", 1.5, 10, 4, "")})


if __name__ == "__main__":
    unittest.main()
