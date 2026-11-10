"""Tests for the reference download logic. They never use the network."""

import os
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import reference  # noqa: E402


def git(cwd, *args):
    return subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.com", *args],
        cwd=cwd, check=True, capture_output=True, text=True,
    ).stdout.strip()


class FetchTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.tmp = Path(self._tmp.name)
        self.origin = self.tmp / "origin"
        self.origin.mkdir()
        git(self.origin, "init", "-q")
        (self.origin / "a.txt").write_text("one\n")
        git(self.origin, "add", "a.txt")
        git(self.origin, "commit", "-q", "-m", "first")
        self.first = git(self.origin, "rev-parse", "HEAD")
        (self.origin / "a.txt").write_text("two\n")
        git(self.origin, "commit", "-q", "-am", "second")
        self.second = git(self.origin, "rev-parse", "HEAD")
        self.dest = self.tmp / "data" / "checkout"

    def repo(self, commit, dest=None):
        return reference.Repo(str(self.origin), commit, dest or self.dest)

    def test_fetch_creates_checkout_at_pinned_commit(self):
        reference.fetch([self.repo(self.first)])
        self.assertEqual(git(self.dest, "rev-parse", "HEAD"), self.first)
        self.assertEqual((self.dest / "a.txt").read_text(), "one\n")

    def test_fetch_is_idempotent(self):
        reference.fetch([self.repo(self.first)])
        marker = self.dest / "a.txt"
        before = marker.stat().st_mtime_ns
        reference.fetch([self.repo(self.first)])
        self.assertEqual(marker.stat().st_mtime_ns, before)
        self.assertEqual(git(self.dest, "rev-parse", "HEAD"), self.first)

    def test_refuses_a_checkout_at_another_commit(self):
        reference.fetch([self.repo(self.first)])
        with self.assertRaises(reference.ReferenceError):
            reference.fetch([self.repo(self.second)])
        self.assertEqual(git(self.dest, "rev-parse", "HEAD"), self.first)

    def test_refuses_a_dirty_checkout(self):
        reference.fetch([self.repo(self.first)])
        (self.dest / "a.txt").write_text("edited\n")
        with self.assertRaises(reference.ReferenceError):
            reference.fetch([self.repo(self.first)])
        self.assertEqual((self.dest / "a.txt").read_text(), "edited\n")

    def test_untracked_files_do_not_make_a_checkout_dirty(self):
        reference.fetch([self.repo(self.first)])
        (self.dest / "output.tet").write_text("generated\n")
        reference.fetch([self.repo(self.first)])

    def test_a_bad_repository_fails_before_anything_is_touched(self):
        other = self.tmp / "data" / "other"
        reference.fetch([self.repo(self.first)])
        with self.assertRaises(reference.ReferenceError):
            reference.fetch([self.repo(self.second), self.repo(self.first, other)])
        self.assertFalse(other.exists())

    def test_unknown_commit_leaves_no_partial_checkout(self):
        with self.assertRaises(reference.ReferenceError):
            reference.fetch([self.repo("0" * 40)])
        self.assertFalse(self.dest.exists())


class BuildTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.source = Path(self._tmp.name) / "src"
        self.source.mkdir()
        git(self.source, "init", "-q")
        (self.source / "CMakeLists.txt").write_text("# original\n")
        git(self.source, "add", "CMakeLists.txt")
        git(self.source, "commit", "-q", "-m", "other")

    def test_build_refuses_a_checkout_at_another_commit(self):
        with mock.patch.object(reference, "_run") as run:
            with self.assertRaises(reference.ReferenceError):
                reference.build_cdt(self.source, Path(self._tmp.name) / "b")
        run.assert_not_called()

    def test_recording_build_refuses_a_checkout_at_another_commit(self):
        build = Path(self._tmp.name) / "b"
        with mock.patch.object(reference, "_run") as run:
            with self.assertRaises(reference.ReferenceError):
                reference.build_recording_cdt(self.source, build)
        run.assert_not_called()
        self.assertFalse(build.exists())

    def test_build_refuses_a_dirty_checkout(self):
        head = git(self.source, "rev-parse", "HEAD")
        (self.source / "CMakeLists.txt").write_text("# edited\n")
        with mock.patch.object(reference, "CDT_COMMIT", head):
            with mock.patch.object(reference, "_run") as run:
                with self.assertRaises(reference.ReferenceError):
                    reference.build_cdt(self.source, Path(self._tmp.name) / "b")
        run.assert_not_called()


class ConstantsTests(unittest.TestCase):
    def test_pinned_hashes_are_full_and_match_the_short_ones(self):
        self.assertEqual(len(reference.CDT_COMMIT), 40)
        self.assertEqual(len(reference.IP_COMMIT), 40)
        self.assertTrue(reference.CDT_COMMIT.startswith("2a71f943b5ff"))
        self.assertTrue(reference.IP_COMMIT.startswith("f8f243d18a68"))
        self.assertEqual(len(reference.NFG_COMMIT), 40)
        self.assertTrue(reference.NFG_COMMIT.startswith("aeec97547a67"))

    def test_fetch_covers_the_three_references(self):
        self.assertEqual(
            [(repo.url, repo.commit, repo.dest) for repo in reference.REPOS],
            [
                (reference.CDT_URL, reference.CDT_COMMIT, reference.CDT_DIR),
                (reference.IP_URL, reference.IP_COMMIT, reference.IP_DIR),
                (reference.NFG_URL, reference.NFG_COMMIT, reference.NFG_DIR),
            ],
        )
        self.assertEqual(reference.NFG_DIR, reference.REFERENCE_DIR / "nfg")

    def test_data_dir_defaults_to_the_corpus_directory(self):
        # Import the module afresh without CORPUS_DATA_DIR, which may be set
        # in the environment of the tests.
        env = {k: v for k, v in os.environ.items() if k != "CORPUS_DATA_DIR"}
        out = subprocess.run([sys.executable, "-c", "import reference; print(reference.DATA_DIR)"],
                             cwd=TOOLS, env=env, capture_output=True, text=True,
                             check=True).stdout.strip()
        self.assertEqual(Path(out), (TOOLS.parent / "data").resolve())


if __name__ == "__main__":
    unittest.main()
