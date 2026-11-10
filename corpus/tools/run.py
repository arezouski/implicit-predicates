"""Running the reference mesher over the sample models.

Each run happens in its own scratch directory under ``corpus/data/runs/``,
with a time limit. A run is valid only when the mesher exits with status 0,
writes a non-empty ``.tet`` file and, for the recording mesher, writes its
corpus file: the mesher's error routine also exits with status 0, and the
recording mesher then writes no corpus.
"""

import csv
import os
import re
import resource
import shutil
import signal
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

from reference import CDT_BUILD_DIR, DATA_DIR, RECORDING_BUILD_DIR, ReferenceError

RUNS_DIR = DATA_DIR / "runs"
CORPUS_OUT_DIR = DATA_DIR / "corpus"
VALIDITY_LOG = RUNS_DIR / "original.csv"
RUN_LOG = CORPUS_OUT_DIR / "run.csv"
ORIGINAL_CDT = CDT_BUILD_DIR / "cdt"
RECORDING_CDT = RECORDING_BUILD_DIR / "cdt"

DEFAULT_TIMEOUT = 600
DEFAULT_JOBS = max(1, (os.cpu_count() or 1) * 3 // 4)

# The summary the recording mesher prints to standard error on exit.
SUMMARY = re.compile(
    r"^ipcorpus: (\d+) calls recorded, (\d+) stored, (\d+) points; (\d+) cascade entries$",
    re.MULTILINE,
)
ABANDONED = "ipcorpus: recording abandoned"

LOG_COLUMNS = ["file_id", "status", "seconds", "calls", "stored", "reason"]


@dataclass(frozen=True)
class Outcome:
    """The result of one mesher run."""

    file_id: int
    status: str  # "ok", "failed" or "timeout"
    seconds: float
    calls: int = 0
    stored: int = 0
    reason: str = ""

    def row(self):
        return [self.file_id, self.status, f"{self.seconds:.2f}", self.calls,
                self.stored, self.reason]


def _without_core_dumps():
    """Run in the mesher's child process before it starts: many models crash
    the reference mesher, and their core dumps are of no use here. A zero
    core size limit survives the ``exec`` and makes systemd-coredump drop the
    dump."""
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def _execute(cdt, model, workdir, timeout, env):
    """Run ``cdt`` on a link to ``model`` inside ``workdir``.

    Returns the exit status (``None`` on timeout), the elapsed seconds, the
    standard error text and the path of the ``.tet`` output.
    """
    shutil.rmtree(workdir, ignore_errors=True)
    workdir.mkdir(parents=True)
    link = workdir / model.name
    link.symlink_to(model.resolve())
    start = time.monotonic()
    with open(workdir / "stdout.txt", "wb") as out, open(workdir / "stderr.txt", "wb") as err:
        process = subprocess.Popen(
            [str(cdt), link.name], cwd=workdir, stdout=out, stderr=err,
            env=env, start_new_session=True, preexec_fn=_without_core_dumps,
        )
        try:
            status = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            status = None
    seconds = time.monotonic() - start
    stderr = (workdir / "stderr.txt").read_text(errors="replace")
    return status, seconds, stderr, workdir / f"{model.name}.tet"


def _failure_reason(status, tet):
    if status is None:
        return "timeout"
    if status < 0:
        try:
            return f"signal {signal.Signals(-status).name}"
        except ValueError:
            return f"signal {-status}"
    if status != 0:
        return f"exit status {status}"
    if not tet.exists() or tet.stat().st_size == 0:
        return "no .tet output"
    return ""


def _finish(workdir, tet, ok):
    """Remove a successful run's directory; keep a failed one's logs only."""
    if ok:
        shutil.rmtree(workdir, ignore_errors=True)
    else:
        tet.unlink(missing_ok=True)


def run_original(file_id, model, timeout, cdt=ORIGINAL_CDT, runs_dir=RUNS_DIR):
    """Run the original mesher and decide whether the model is valid."""
    workdir = runs_dir / "original" / str(file_id)
    status, seconds, _, tet = _execute(cdt, model, workdir, timeout, None)
    reason = _failure_reason(status, tet)
    state = "timeout" if status is None else "failed" if reason else "ok"
    _finish(workdir, tet, not reason)
    return Outcome(file_id, state, seconds, reason=reason)


def run_recording(file_id, model, timeout, out, cap=None,
                  cdt=RECORDING_CDT, runs_dir=RUNS_DIR):
    """Run the recording mesher; on success its corpus is moved to ``out``."""
    workdir = runs_dir / "recording" / str(file_id)
    corpus = workdir / "calls.ipcorpus"
    env = dict(os.environ, IPCORPUS_OUT=str(corpus), IPCORPUS_MODEL=f"thingi10k-{file_id}")
    if cap is not None:
        env["IPCORPUS_STRATUM_CAP"] = str(cap)
    status, seconds, stderr, tet = _execute(cdt, model, workdir, timeout, env)
    reason = "abandoned" if ABANDONED in stderr else _failure_reason(status, tet)
    if not reason and not corpus.exists():
        reason = "no corpus output"
    calls = stored = 0
    if not reason:
        summary = SUMMARY.search(stderr)
        if summary is None:
            raise ReferenceError(f"model {file_id}: the recorder printed no summary")
        calls, stored, _, cascades = map(int, summary.groups())
        if calls != cascades:
            raise ReferenceError(
                f"model {file_id}: {calls} recorded calls but {cascades} cascade "
                "entries; a call path bypasses the recorder"
            )
        out.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(corpus, out)
    state = "timeout" if status is None else "failed" if reason else "ok"
    _finish(workdir, tet, not reason)
    return Outcome(file_id, state, seconds, calls, stored, reason)


def within(outcome, timeout):
    """Whether a run succeeded within ``timeout`` seconds."""
    return outcome.status == "ok" and outcome.seconds <= timeout


def settled(outcome, timeout):
    """Whether an earlier run decides the outcome under ``timeout``: any run
    but one that timed out under a shorter limit."""
    return outcome is not None and (outcome.status != "timeout" or outcome.seconds >= timeout)


def run_parallel(task, items, jobs, label):
    """Apply ``task`` to every item with ``jobs`` workers, printing progress."""
    outcomes = []
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        for outcome in pool.map(task, items):
            outcomes.append(outcome)
            print(f"{label} {len(outcomes)}/{len(items)}: model {outcome.file_id} "
                  f"{outcome.status} in {outcome.seconds:.1f} s {outcome.reason}",
                  flush=True)
    return outcomes


def read_log(path):
    """The outcomes of a log, by file id; empty when it does not exist."""
    if not path.exists():
        return {}
    with open(path, newline="") as f:
        return {
            int(r["file_id"]): Outcome(int(r["file_id"]), r["status"], float(r["seconds"]),
                                       int(r["calls"]), int(r["stored"]), r["reason"])
            for r in csv.DictReader(f)
        }


def write_log(path, outcomes):
    """Write the outcomes, sorted by file id, replacing the file atomically."""
    path.parent.mkdir(parents=True, exist_ok=True)
    partial = path.with_name(path.name + ".partial")
    with open(partial, "w", newline="") as f:
        writer = csv.writer(f, lineterminator="\n")
        writer.writerow(LOG_COLUMNS)
        for outcome in sorted(outcomes.values(), key=lambda o: o.file_id):
            writer.writerow(outcome.row())
    partial.replace(path)


if __name__ == "__main__":
    sys.exit("run corpus.py instead")
