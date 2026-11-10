"""Tests of the recorder alone, through a small driver program.

They need a C++20 compiler and are skipped, with a message, without one.
"""

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import instrument  # noqa: E402

COMPILER = shutil.which("c++") or shutil.which("g++") or shutil.which("clang++")

# Minimal point types with the interface the recorder reads.
DRIVER = r"""
#include "recorder.hpp"
#include <cstdlib>
#include <cstring>

struct Explicit {
    double x, y, z;
    double X() const { return x; }
    double Y() const { return y; }
    double Z() const { return z; }
};
struct Lnc {
    Explicit p, q;
    const Explicit& P() const { return p; }
    const Explicit& Q() const { return q; }
    double T() const { return 0.5; }
};
struct Lpi {
    Explicit e;
    const Explicit& P() const { return e; }
    const Explicit& Q() const { return e; }
    const Explicit& R() const { return e; }
    const Explicit& S() const { return e; }
    const Explicit& T() const { return e; }
};
struct Tpi {
    Explicit e;
    const Explicit& V1() const { return e; }
    const Explicit& V2() const { return e; }
    const Explicit& V3() const { return e; }
    const Explicit& W1() const { return e; }
    const Explicit& W2() const { return e; }
    const Explicit& W3() const { return e; }
    const Explicit& U1() const { return e; }
    const Explicit& U2() const { return e; }
    const Explicit& U3() const { return e; }
};
struct Point {
    Explicit e;
    bool isExplicit3D() const { return true; }
    bool isLNC() const { return false; }
    bool isLPI() const { return false; }
    bool isTPI() const { return false; }
    const Explicit& toExplicit3D() const { return e; }
    const Lnc& toLNC() const { static Lnc l{}; return l; }
    const Lpi& toLPI() const { static Lpi l{}; return l; }
    const Tpi& toTPI() const { static Tpi t{}; return t; }
};

static int positive(const Point&, const Point&, const Point&, const Point&) {
    ipcorpus::mark_stage(ipcorpus::Stage::Filtered);
    return 1;
}

int main(int argc, char** argv) {
    const char* mode = argc > 1 ? argv[1] : "ok";
    if (std::strcmp(mode, "abandon-first") == 0) {
        ipcorpus::abandon();
        std::exit(0);
    }
    const Point a{{1, 0, 0}}, b{{0, 1, 0}}, c{{0, 0, 1}}, d{{0, 0, 0}};
    for (int i = 0; i < 3; i++) ipcorpus::record(ipcorpus::Predicate::Orient3d, positive, a, b, c, d);
    if (std::strcmp(mode, "abandon") == 0) {
        ipcorpus::abandon();
        std::exit(0);
    }
    if (std::strcmp(mode, "crash") == 0) std::abort();
    return 0;
}
"""


@unittest.skipIf(COMPILER is None, "no C++ compiler found")
class RecorderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls._tmp = tempfile.TemporaryDirectory()
        cls.tmp = Path(cls._tmp.name)
        driver = cls.tmp / "driver.cpp"
        driver.write_text(DRIVER)
        cls.exe = cls.tmp / "driver"
        subprocess.run(
            [COMPILER, "-std=c++20", "-O1", f"-I{instrument.RECORDER_DIR}", str(driver),
             str(instrument.RECORDER_DIR / "recorder.cpp"), "-o", str(cls.exe)],
            check=True,
        )

    @classmethod
    def tearDownClass(cls):
        cls._tmp.cleanup()

    def setUp(self):
        self.out = self.tmp / "calls.ipcorpus"
        self.partial = self.tmp / "calls.ipcorpus.partial"
        for path in (self.out, self.partial):
            path.unlink(missing_ok=True)

    def run_driver(self, mode):
        env = {"IPCORPUS_OUT": str(self.out), "IPCORPUS_MODEL": "toy"}
        return subprocess.run([str(self.exe), mode], env=env, capture_output=True, text=True)

    def leave_stale_files(self):
        self.out.write_bytes(b"stale")
        self.partial.write_bytes(b"stale")

    def test_a_normal_exit_writes_the_corpus(self):
        result = self.run_driver("ok")
        self.assertEqual(result.returncode, 0)
        self.assertTrue(self.out.read_bytes().startswith(b"IPCORPUS"))
        self.assertFalse(self.partial.exists())
        self.assertIn("3 calls recorded", result.stderr)

    def test_an_abandoned_run_leaves_no_file(self):
        self.leave_stale_files()
        result = self.run_driver("abandon")
        self.assertEqual(result.returncode, 0)
        self.assertFalse(self.out.exists())
        self.assertFalse(self.partial.exists())
        self.assertIn("recording abandoned", result.stderr)

    def test_abandoning_before_any_call_leaves_no_file(self):
        self.leave_stale_files()
        result = self.run_driver("abandon-first")
        self.assertEqual(result.returncode, 0)
        self.assertFalse(self.out.exists())
        self.assertFalse(self.partial.exists())
        self.assertIn("recording abandoned", result.stderr)

    def test_a_crashed_run_removes_the_stale_file(self):
        self.leave_stale_files()
        result = self.run_driver("crash")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.out.exists())
        self.assertFalse(self.partial.exists())


if __name__ == "__main__":
    unittest.main()
