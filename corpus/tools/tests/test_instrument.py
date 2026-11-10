"""Tests for the instrumentation of the reference mesher, on toy source trees."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import instrument  # noqa: E402


def definitions(header, count):
    return f"inline int {header}(double x)\n{{\n\treturn 0;\n}}\n" * count


def write_toy_tree(root):
    """Write a tree with every anchor once and every family at its count."""
    files = {
        "CMakeLists.txt": "project(cdt)\nadd_executable(${PROJECT_NAME}\n    src/main.cpp\n)\n",
        "src/main.cpp": "int main() { return 0; }\n",
        "include/implicit_point.h": (
            "class genericPoint {\n"
            f"\t{instrument.ORIENT3D_DECLARATION}\n"
            f"\t{instrument.INSPHERE_DECLARATION}\n"
            "};\n"
        ),
        "include/implicit_point.hpp": (
            "inline int genericPoint::orient3D(const genericPoint& a)\n{\n\treturn 0;\n}\n"
            + "inline int genericPoint::inSphere(const genericPoint& a)\n{\n\treturn 0;\n}\n" * 2
        ),
    }
    for path in instrument.STAGE_FUNCTIONS:
        files[path] = definitions("orient3d_exact", instrument.STAGE_FUNCTIONS[path]) + definitions(
            "inSphere", instrument.DISPATCHERS[path]
        )
    for path, text in files.items():
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        (root / path).write_text(text)


def snapshot(root):
    return {p.relative_to(root): p.read_bytes() for p in sorted(root.rglob("*")) if p.is_file()}


class InstrumentTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.source = Path(self._tmp.name) / "source"
        self.dest = Path(self._tmp.name) / "copy"
        write_toy_tree(self.source)

    def edit(self, path, old, new):
        file = self.source / path
        text = file.read_text()
        self.assertIn(old, text)
        file.write_text(text.replace(old, new))

    def test_hooks_are_inserted(self):
        instrument.prepare(self.source, self.dest)
        everything = "".join(p.read_text() for p in self.dest.rglob("*") if p.is_file())
        stages = sum(instrument.STAGE_FUNCTIONS.values())
        dispatchers = sum(instrument.DISPATCHERS.values())
        self.assertEqual(everything.count("ipcorpus::mark_stage("), stages)
        self.assertEqual(everything.count("ipcorpus::count_low_level()"), dispatchers)
        self.assertEqual(everything.count("inline int genericPoint::orient3D_unrecorded("), 1)
        self.assertEqual(everything.count("inline int genericPoint::inSphere_unrecorded("), 2)
        self.assertEqual(everything.count("inline int genericPoint::orient3D("), 1)
        self.assertEqual(everything.count("inline int genericPoint::inSphere("), 1)
        self.assertIn("src/recorder.cpp", (self.dest / "CMakeLists.txt").read_text())
        self.assertTrue((self.dest / "include" / "recorder.hpp").is_file())
        self.assertTrue((self.dest / "src" / "recorder.cpp").is_file())

    def test_missing_anchor_fails_naming_it(self):
        self.edit("include/implicit_point.h", instrument.ORIENT3D_DECLARATION, "")
        with self.assertRaises(instrument.InstrumentError) as raised:
            instrument.prepare(self.source, self.dest)
        self.assertIn(instrument.ORIENT3D_DECLARATION, str(raised.exception))

    def test_duplicated_anchor_fails(self):
        line = instrument.INSPHERE_DECLARATION
        self.edit("include/implicit_point.h", line, f"{line}\n{line}")
        with self.assertRaises(instrument.InstrumentError) as raised:
            instrument.prepare(self.source, self.dest)
        self.assertIn(line, str(raised.exception))

    def test_wrong_family_count_fails(self):
        self.edit(
            "include/indirect_predicates.hpp",
            "inline int orient3d_exact(double x)\n{\n\treturn 0;\n}\n",
            "",
        )
        with self.assertRaises(instrument.InstrumentError) as raised:
            instrument.prepare(self.source, self.dest)
        self.assertIn("include/indirect_predicates.hpp", str(raised.exception))

    def test_instrumenting_twice_fails(self):
        instrument.prepare(self.source, self.dest)
        before = snapshot(self.dest)
        with self.assertRaises(instrument.InstrumentError):
            instrument.instrument(self.dest)
        self.assertEqual(snapshot(self.dest), before)

    def test_original_tree_is_untouched(self):
        before = snapshot(self.source)
        instrument.prepare(self.source, self.dest)
        self.assertEqual(snapshot(self.source), before)

    def test_a_failed_instrumentation_leaves_no_copy(self):
        self.edit("CMakeLists.txt", "add_executable", "add_library")
        with self.assertRaises(instrument.InstrumentError):
            instrument.prepare(self.source, self.dest)
        self.assertFalse(self.dest.exists())


if __name__ == "__main__":
    unittest.main()
