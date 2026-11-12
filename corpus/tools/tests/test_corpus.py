"""Tests of the command-line entry point, with the steps it runs replaced."""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import corpus  # noqa: E402
import sample  # noqa: E402


class ArgumentTests(unittest.TestCase):
    def test_defaults_are_the_committed_sample(self):
        args = corpus.parse(["select"])
        self.assertEqual((args.count, args.manifest), (sample.TARGET, corpus.MANIFEST))

    def test_count_and_manifest_can_be_chosen(self):
        args = corpus.parse(["all", "--count", "8", "--manifest", "/x/m.csv"])
        self.assertEqual((args.count, args.manifest), (8, Path("/x/m.csv")))

    def test_fetch_models_takes_a_manifest_but_no_count(self):
        args = corpus.parse(["fetch-models", "--manifest", "/x/m.csv"])
        self.assertEqual(args.manifest, Path("/x/m.csv"))
        self.assertFalse(hasattr(args, "count"))


    def test_the_corpus_directory_and_the_population_can_be_chosen(self):
        args = corpus.parse(["run"])
        self.assertEqual((args.corpus_dir, args.population), (None, "all"))
        args = corpus.parse(["run", "--corpus-dir", "/x/full", "--population", "valid"])
        self.assertEqual((args.corpus_dir, args.population), (Path("/x/full"), "valid"))


class AllTests(unittest.TestCase):
    def test_all_runs_every_step_in_order(self):
        steps = []
        names = ["fetch_reference", "build", "fetch_models", "run_sample"]
        patches = [mock.patch.object(corpus, n, lambda _a, n=n: steps.append(n)) for n in names]
        for p in patches:
            p.start()
            self.addCleanup(p.stop)
        corpus.all_commands(corpus.parse(["all"]))
        self.assertEqual(steps, names)


class SelectTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)
        metadata = self.tmp / "metadata.csv"
        metadata.write_text("")
        p = mock.patch.object(corpus.models, "METADATA", metadata)
        p.start()
        self.addCleanup(p.stop)
        for target in ["VALIDITY_LOG", "RUN_LOG"]:
            p = mock.patch.object(corpus.run, target, self.tmp / f"{target}.csv")
            p.start()
            self.addCleanup(p.stop)

    def test_select_uses_the_count_and_writes_the_chosen_manifest(self):
        manifest = self.tmp / "m.csv"
        calls = {}

        def select(_models, target, seed, _valid, lookahead):
            calls["select"] = (target, seed)
            return sample.Selection([], [])

        with mock.patch.object(sample, "select", select), \
                mock.patch.object(sample, "write_manifest",
                                  lambda path, models: calls.update(write=path)):
            corpus.select(corpus.parse(["select", "--count", "8", "--manifest", str(manifest)]))
        self.assertEqual(calls, {"select": (8, sample.SEED), "write": manifest})


    def models(self):
        return [sample.Model(1, "CC0", 10, 10, True, False),
                sample.Model(2, "CC0", 10, 10, True, True)]

    def selected(self, *options, original=None, recording=None, unexportable=()):
        """The file ids `select` keeps with the given options and run logs,
        when the models in `unexportable` get no OFF file."""
        corpus.run.write_log(corpus.run.VALIDITY_LOG, original or {})
        corpus.run.write_log(corpus.run.RUN_LOG, recording or {})

        def run_original(file_id, _model, _timeout):
            return corpus.run.Outcome(file_id, "ok", 1.0, digest="d" * 64)

        def export(file_ids):
            for file_id in file_ids:
                if file_id not in unexportable:
                    (self.tmp / f"{file_id}.off").write_text("OFF")

        with mock.patch.object(sample, "read_metadata", lambda _path: self.models()), \
                mock.patch.object(corpus.models, "MODELS_DIR", self.tmp), \
                mock.patch.object(corpus.models, "export", export), \
                mock.patch.object(corpus.run, "run_original", run_original):
            args = corpus.parse(["select", "--jobs", "1", "--manifest", str(self.tmp / "m.csv"),
                                 *options])
            selection, _ = corpus._select(args)
        return [m.file_id for m in selection.models]

    def ok(self, file_id):
        return corpus.run.Outcome(file_id, "ok", 1.0, digest="d" * 64)

    def test_a_population_keeps_only_its_models(self):
        original = {1: self.ok(1), 2: self.ok(2)}
        self.assertEqual(self.selected(original=original), [1, 2])
        self.assertEqual(self.selected("--population", "valid", original=original), [1])
        self.assertEqual(self.selected("--population", "self-intersecting", original=original),
                         [2])

    def test_a_model_without_an_off_file_is_not_valid(self):
        self.assertEqual(self.selected(unexportable=[1]), [2])
        logged = corpus.run.read_log(corpus.run.VALIDITY_LOG)
        self.assertEqual((logged[1].status, logged[1].reason), ("failed", "no OFF file"))
        self.assertEqual(logged[2].status, "ok")

    def test_a_recording_that_failed_excludes_its_model(self):
        failed = corpus.run.Outcome(1, "failed", 1.0, reason="output differs")
        self.assertEqual(self.selected(original={1: self.ok(1), 2: self.ok(2)},
                                       recording={1: failed}), [2])

    def test_a_recording_that_timed_out_under_a_shorter_limit_is_tried_again(self):
        original = {1: self.ok(1), 2: self.ok(2)}
        timeout = corpus.run.Outcome(1, "timeout", 60.0, reason="timeout")
        self.assertEqual(self.selected("--timeout", "60", original=original,
                                       recording={1: timeout}), [2])
        self.assertEqual(self.selected("--timeout", "600", original=original,
                                       recording={1: timeout}), [1, 2])


class RunSampleTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)
        self.out = self.tmp / "corpus"
        self.out.mkdir()
        for target, value in [("RUN_LOG", self.out / "run.csv"), ("CORPUS_OUT_DIR", self.out)]:
            p = mock.patch.object(corpus.run, target, value)
            p.start()
            self.addCleanup(p.stop)
        self.model = sample.Model(1, "CC0", 10, 10, True, False)
        self.original = {1: corpus.run.Outcome(1, "ok", 1.0, digest="d" * 64)}
        p = mock.patch.object(corpus, "_select", lambda _args: (
            sample.Selection([self.model], []), self.original))
        p.start()
        self.addCleanup(p.stop)
        self.recorded = []

    def record(self, file_id, _model, _timeout, out, expected):
        self.recorded.append((file_id, expected))
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text("corpus")
        return corpus.run.Outcome(file_id, "ok", 1.0, 7, 5, digest=expected)

    def run_sample(self):
        with mock.patch.object(corpus.run, "run_recording", self.record):
            corpus.run_sample(corpus.parse(["run", "--jobs", "1"]))

    def test_a_recording_logged_without_digest_is_recorded_again(self):
        (self.out / "1.ipcorpus").write_text("old")
        corpus.run.write_log(corpus.run.RUN_LOG, {1: corpus.run.Outcome(1, "ok", 1.0, 7, 5)})
        self.run_sample()
        self.assertEqual(self.recorded, [(1, "d" * 64)])
        self.assertEqual((self.out / "1.ipcorpus").read_text(), "corpus")

    def test_a_current_recording_is_not_run_again(self):
        (self.out / "1.ipcorpus").write_text("kept")
        corpus.run.write_log(corpus.run.RUN_LOG,
                             {1: corpus.run.Outcome(1, "ok", 1.0, 7, 5, digest="d" * 64)})
        self.run_sample()
        self.assertEqual(self.recorded, [])

    def test_a_chosen_corpus_directory_holds_the_files_and_its_own_log(self):
        full = self.tmp / "full"
        with mock.patch.object(corpus.run, "run_recording", self.record):
            corpus.run_sample(corpus.parse(["run", "--jobs", "1", "--corpus-dir", str(full)]))
        self.assertEqual(sorted(p.name for p in full.iterdir()), ["1.ipcorpus", "run.csv"])
        self.assertEqual(list(self.out.iterdir()), [])

    def test_corpus_files_of_models_not_in_the_manifest_are_removed(self):
        (self.out / "2.ipcorpus").write_text("replaced model")
        self.run_sample()
        self.assertEqual(sorted(p.name for p in self.out.glob("*.ipcorpus")), ["1.ipcorpus"])


class DataDirTests(unittest.TestCase):
    def test_the_data_directory_can_be_moved(self):
        with tempfile.TemporaryDirectory() as tmp:
            code = ("import models, reference, run; "
                    "print(reference.DATA_DIR, reference.REFERENCE_DIR.parent, "
                    "run.RUNS_DIR.parent, run.CORPUS_OUT_DIR.parent, models.CACHE_DIR.parent)")
            env = dict(os.environ, CORPUS_DATA_DIR=tmp)
            out = subprocess.run([sys.executable, "-c", code], cwd=TOOLS, env=env,
                                 capture_output=True, text=True, check=True).stdout
            self.assertEqual(out.split(), [str(Path(tmp).resolve())] * 5)


if __name__ == "__main__":
    unittest.main()
