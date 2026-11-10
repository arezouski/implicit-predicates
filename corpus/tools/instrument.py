"""Instrument a copy of the reference mesher so that it records its predicate calls.

The copy gets the recorder (``corpus/tools/recorder/``) and a few one-line
hooks. Each hook is located either by a one-line anchor, which must appear
exactly once, or by a regular expression over function definitions, which
must match a fixed number of times. Any other count fails, naming the hook,
before a single file is written. The source tree is never modified.
"""

import re
import shutil
from dataclasses import dataclass
from pathlib import Path

RECORDER_DIR = Path(__file__).resolve().parent / "recorder"

# Written into the root of an instrumented tree.
MARKER = "IPCORPUS_INSTRUMENTED"

IMPLICIT_POINT_H = "include/implicit_point.h"
IMPLICIT_POINT_HPP = "include/implicit_point.hpp"
HAND_OPTIMIZED = "include/hand_optimized_predicates.hpp"
INDIRECT = "include/indirect_predicates.hpp"

ORIENT3D_DECLARATION = (
    "static int orient3D(const genericPoint& a, const genericPoint& b, "
    "const genericPoint& c, const genericPoint& d);"
)
INSPHERE_DECLARATION = (
    "static int inSphere(const genericPoint& a, const genericPoint& b, "
    "const genericPoint& c, const genericPoint& d, const genericPoint& e);"
)
EXECUTABLE_ANCHOR = "add_executable(${PROJECT_NAME}"

# Definitions of the two predicates the mesher calls. ``inSphere`` has two
# alternative definitions selected by the preprocessor; both are renamed.
WRAPPED_DEFINITION = r"^inline int genericPoint::(orient3D|inSphere)\("
WRAPPED_COUNT = 3

# Stage functions of the orient3d and insphere cascades, per file.
STAGE_DEFINITION = r"^inline int (?:orient3d|inSphere)\w*_(filtered|interval|bigfloat|exact)\("
STAGE_FUNCTIONS = {HAND_OPTIMIZED: 3, INDIRECT: 31}

# The entry function of each cascade (one per combination of explicit and
# implicit arguments), per file.
DISPATCHER_DEFINITION = r"^inline int (?:orient3d|inSphere)(?:_indirect_[IE]{4}|_[IE]{5})?\("
DISPATCHERS = {HAND_OPTIMIZED: 1, INDIRECT: 10}

STAGES = {"filtered": "Filtered", "interval": "Interval", "bigfloat": "Bigfloat", "exact": "Exact"}

INCLUDE = '#include "recorder.hpp"\n'

WRAPPERS = """
// Corpus recorder: every call of the two predicates goes through it.
inline int genericPoint::orient3D(const genericPoint& a, const genericPoint& b, const genericPoint& c, const genericPoint& d) { return ipcorpus::record(ipcorpus::Predicate::Orient3d, &genericPoint::orient3D_unrecorded, a, b, c, d); }
inline int genericPoint::inSphere(const genericPoint& a, const genericPoint& b, const genericPoint& c, const genericPoint& d, const genericPoint& e) { return ipcorpus::record(ipcorpus::Predicate::InSphere, &genericPoint::inSphere_unrecorded, a, b, c, d, e); }
"""


class InstrumentError(Exception):
    """A hook could not be placed, or the tree is already instrumented."""


def _line_after(text):
    """An edit that adds ``text`` on a new line after the matched one, with
    the same indentation."""

    def edit(source, match):
        start = source.rfind("\n", 0, match.start()) + 1
        end = source.find("\n", match.end())
        end = len(source) if end < 0 else end
        indent = re.match(r"[ \t]*", source[start:]).group(0)
        return source[:end] + "\n" + indent + text + source[end:]

    return edit


def _in_body(statement):
    """An edit that adds ``statement(match)`` right after the opening brace of
    the function whose definition was matched."""

    def edit(source, match):
        brace = source.find("{", match.end())
        if brace < 0:
            raise InstrumentError(f"no body after {match.group(0)!r}")
        return source[: brace + 1] + " " + statement(match) + source[brace + 1 :]

    return edit


def _rename(suffix):
    """An edit that appends ``suffix`` to the name in the first group."""

    def edit(source, match):
        return source[: match.end(1)] + suffix + source[match.end(1) :]

    return edit


@dataclass(frozen=True)
class Hook:
    """Where a hook goes and how many times its pattern must match."""

    name: str
    path: str
    pattern: str
    count: int
    edit: object


def _anchor(path, anchor, edit):
    return Hook(anchor, path, r"^.*" + re.escape(anchor) + r".*$", 1, edit)


def _mark_stage(match):
    return f"ipcorpus::mark_stage(ipcorpus::Stage::{STAGES[match.group(1)]});"


HOOKS = [
    _anchor(IMPLICIT_POINT_H, ORIENT3D_DECLARATION,
            _line_after(ORIENT3D_DECLARATION.replace("orient3D(", "orient3D_unrecorded("))),
    _anchor(IMPLICIT_POINT_H, INSPHERE_DECLARATION,
            _line_after(INSPHERE_DECLARATION.replace("inSphere(", "inSphere_unrecorded("))),
    Hook("definitions of genericPoint::orient3D and inSphere", IMPLICIT_POINT_HPP,
         WRAPPED_DEFINITION, WRAPPED_COUNT, _rename("_unrecorded")),
    _anchor("CMakeLists.txt", EXECUTABLE_ANCHOR, _line_after("    src/recorder.cpp")),
]
for _path, _count in STAGE_FUNCTIONS.items():
    HOOKS.append(Hook("stage functions", _path, STAGE_DEFINITION, _count, _in_body(_mark_stage)))
for _path, _count in DISPATCHERS.items():
    HOOKS.append(Hook("cascade entry functions", _path, DISPATCHER_DEFINITION, _count,
                      _in_body(lambda _match: "ipcorpus::count_low_level();")))

# Files that use the recorder and include it first.
USERS = [IMPLICIT_POINT_H, IMPLICIT_POINT_HPP, HAND_OPTIMIZED, INDIRECT]


def _apply(source, hook):
    matches = list(re.finditer(hook.pattern, source, re.MULTILINE))
    if len(matches) != hook.count:
        raise InstrumentError(
            f"{hook.path}: {hook.name!r} matched {len(matches)} times, "
            f"expected {hook.count}"
        )
    for match in reversed(matches):
        source = hook.edit(source, match)
    return source


def instrument(tree, recorder_dir=RECORDER_DIR):
    """Instrument the source tree ``tree`` in place.

    Every hook is placed in memory first; files are written only if all of
    them succeed. Fails if the tree is already instrumented.
    """
    tree = Path(tree)
    if (tree / MARKER).exists():
        raise InstrumentError(f"{tree} is already instrumented")
    # The sources are not all UTF-8; Latin-1 keeps every byte as it is.
    texts = {}
    for hook in HOOKS:
        if hook.path not in texts:
            texts[hook.path] = (tree / hook.path).read_text(encoding="latin-1")
        texts[hook.path] = _apply(texts[hook.path], hook)
    texts[IMPLICIT_POINT_HPP] += WRAPPERS
    for path in USERS:
        texts[path] = INCLUDE + texts[path]
    copies = {"include/recorder.hpp": "recorder.hpp", "src/recorder.cpp": "recorder.cpp"}
    for path in copies:
        if (tree / path).exists():
            raise InstrumentError(f"{tree / path} already exists")
    for path, text in texts.items():
        (tree / path).write_text(text, encoding="latin-1")
    for path, name in copies.items():
        shutil.copyfile(Path(recorder_dir) / name, tree / path)
    (tree / MARKER).write_text("instrumented by corpus/tools/instrument.py\n")


def prepare(source, dest, recorder_dir=RECORDER_DIR):
    """Copy ``source`` to the new directory ``dest`` and instrument the copy.

    The repository metadata and the example models are not copied. On
    failure ``dest`` is removed.
    """
    dest = Path(dest)
    if dest.exists():
        raise InstrumentError(f"{dest} already exists")
    shutil.copytree(source, dest, ignore=shutil.ignore_patterns(".git", "Input_file"))
    try:
        instrument(dest, recorder_dir)
    except BaseException:
        shutil.rmtree(dest, ignore_errors=True)
        raise
