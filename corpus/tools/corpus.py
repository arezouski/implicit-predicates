#!/usr/bin/env python3
"""Single entry point for the corpus tooling.

Add a subcommand by writing a function that takes the parsed arguments and
registering it in ``COMMANDS``.
"""

import argparse
import sys
from pathlib import Path

import models
import reference
import run
import sample

MANIFEST = Path(__file__).resolve().parents[1] / "manifest" / "sample.csv"


def fetch_reference(_args):
    reference.fetch()


def build(_args):
    print(f"built {reference.build_cdt()}")
    print(f"built {reference.build_recording_cdt()}")


def _select(args):
    """Select the sample: a model is valid when the original mesher meshes it
    and the recording mesher has not failed on it (a recording that writes
    another output than the original's is a failure).

    Returns the selection and the log of the original runs."""
    if not models.METADATA.exists():
        raise reference.ReferenceError(f"{models.METADATA} is missing; run fetch-models first")
    original = run.read_log(run.VALIDITY_LOG)
    recording = run.read_log(run.RUN_LOG)

    def check(m):
        return run.run_original(m.file_id, models.model_path(m.file_id), args.timeout)

    def valid(batch):
        todo = [m for m in batch
                if not run.settled(original.get(m.file_id), args.timeout)]
        models.export(m.file_id for m in todo)
        for outcome in run.run_parallel(check, todo, args.jobs, "original"):
            original[outcome.file_id] = outcome
            run.write_log(run.VALIDITY_LOG, original)
        return {
            m.file_id for m in batch
            if run.within(original[m.file_id], args.timeout)
            and recording.get(m.file_id, original[m.file_id]).status == "ok"
        }

    selection = sample.select(sample.read_metadata(models.METADATA),
                              args.count, sample.SEED, valid, lookahead=args.jobs)
    sample.write_manifest(args.manifest, selection.models)
    print(f"selected {len(selection.models)} models into {args.manifest} "
          f"({len(selection.invalid)} candidates were not valid)")
    return selection, original


def fetch_models(args):
    models.fetch_metadata()
    if args.manifest.exists():
        models.export(row["file_id"] for row in sample.read_manifest(args.manifest))


def select(args):
    _select(args)


def _corpus_file(file_id):
    return run.CORPUS_OUT_DIR / f"{file_id}.ipcorpus"


def run_sample(args):
    """Record every manifest model, replacing those the recording mesher
    fails on by the next candidate of their stratum, until all succeed; then
    remove the corpus files of models that are no longer in the manifest."""
    while True:
        selection, original = _select(args)
        recording = run.read_log(run.RUN_LOG)
        todo = [m for m in selection.models
                if not run.current(recording.get(m.file_id), original[m.file_id])
                or not _corpus_file(m.file_id).exists()]
        if not todo:
            break

        def record(m):
            return run.run_recording(m.file_id, models.model_path(m.file_id), args.timeout,
                                     _corpus_file(m.file_id), original[m.file_id].digest)

        for outcome in run.run_parallel(record, todo, args.jobs, "recording"):
            recording[outcome.file_id] = outcome
            run.write_log(run.RUN_LOG, recording)
    removed = run.reconcile(run.CORPUS_OUT_DIR, {m.file_id for m in selection.models})
    failed = [o for o in recording.values() if o.status != "ok"]
    print(f"recorded {len(selection.models)} models into {run.CORPUS_OUT_DIR}; "
          f"{len(failed)} were replaced after failing to record; "
          f"removed the corpus files of {len(removed)} models no longer in the manifest")


def all_commands(args):
    fetch_reference(args)
    build(args)
    fetch_models(args)
    run_sample(args)


COMMANDS = {
    "fetch-reference": (
        fetch_reference,
        "download the reference repositories at their pinned commits",
    ),
    "build": (
        build,
        "build the original reference mesher and the one that records its predicate calls",
    ),
    "fetch-models": (
        fetch_models,
        "download the Thingi10K dataset and write the OFF files of the manifest models",
    ),
    "select": (select, "select the sample models and write the manifest"),
    "run": (run_sample, "record the predicate calls of every manifest model"),
    "all": (all_commands, "fetch-reference, build, fetch-models, select and run, in sequence"),
}

# Commands that run the mesher and take --jobs, --timeout and --count.
RUNNING = {"select", "run", "all"}
# Commands that read or write the manifest and take --manifest.
MANIFEST_USERS = RUNNING | {"fetch-models"}


def parse(argv=None):
    """The parsed command line."""
    parser = argparse.ArgumentParser(
        description=__doc__,
        epilog="The data directory is corpus/data/, or the one named by CORPUS_DATA_DIR.")
    commands = parser.add_subparsers(dest="command", required=True)
    for name, (_, help_text) in COMMANDS.items():
        command = commands.add_parser(name, help=help_text)
        if name in RUNNING:
            command.add_argument("--jobs", type=int, default=run.DEFAULT_JOBS,
                                 help=f"parallel mesher runs (default {run.DEFAULT_JOBS})")
            command.add_argument("--timeout", type=float, default=run.DEFAULT_TIMEOUT,
                                 help=f"seconds per run (default {run.DEFAULT_TIMEOUT})")
            command.add_argument("--count", type=int, default=sample.TARGET,
                                 help=f"number of sample models (default {sample.TARGET})")
        if name in MANIFEST_USERS:
            command.add_argument("--manifest", type=Path, default=MANIFEST,
                                 help="manifest file (default: the committed one)")
    return parser.parse_args(argv)


def main(argv=None):
    args = parse(argv)
    try:
        COMMANDS[args.command][0](args)
    except reference.ReferenceError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
