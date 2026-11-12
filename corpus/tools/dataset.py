"""Access to the Thingi10K dataset, run with the interpreter of its own environment.

This script needs the packages of ``requirements.txt`` and is run by
``corpus.py`` with the Python of ``corpus/data/venv/``. It only reads geometry
and metadata from the dataset, which is treated as untrusted data.

    python dataset.py metadata <cache_dir> <out.csv>
    python dataset.py export <cache_dir> <models_dir> <file_id>...
"""

import csv
import math
import sys
from pathlib import Path

import thingi10k

METADATA_COLUMNS = [
    "file_id",
    "thing_id",
    "author",
    "license",
    "num_vertices",
    "num_facets",
    "closed",
    "self_intersecting",
]


def _init(cache_dir):
    thingi10k.init(variant="npz", cache_dir=str(cache_dir))


def write_metadata(cache_dir, out):
    """Download the dataset if needed and write the metadata of every model."""
    _init(cache_dir)
    rows = thingi10k.dataset().select_columns(METADATA_COLUMNS)
    out = Path(out)
    partial = out.with_name(out.name + ".partial")
    with open(partial, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(METADATA_COLUMNS)
        for row in rows:
            writer.writerow(
                ["" if row[c] is None else int(row[c]) if isinstance(row[c], bool)
                 else row[c] for c in METADATA_COLUMNS]
            )
    partial.replace(out)
    print(f"wrote the metadata of {len(rows)} models to {out}")


def _off_text(vertices, facets):
    """The OFF text of a triangle mesh, rejecting malformed geometry."""
    if vertices.ndim != 2 or vertices.shape[1] != 3:
        raise ValueError(f"vertices have shape {vertices.shape}")
    if facets.ndim != 2 or facets.shape[1] != 3:
        raise ValueError(f"facets have shape {facets.shape}")
    n = len(vertices)
    lines = ["OFF", f"{n} {len(facets)} 0"]
    for x, y, z in vertices.tolist():
        if not all(math.isfinite(c) for c in (x, y, z)):
            raise ValueError("a vertex coordinate is not finite")
        # repr gives the shortest decimal that reads back to the same double.
        lines.append(f"{x!r} {y!r} {z!r}")
    for a, b, c in facets.tolist():
        if not all(0 <= i < n for i in (a, b, c)):
            raise ValueError("a facet refers to a missing vertex")
        lines.append(f"3 {a} {b} {c}")
    return "\n".join(lines) + "\n"


def export(cache_dir, models_dir, file_ids):
    """Write ``<models_dir>/<file_id>.off`` for each requested model."""
    _init(cache_dir)
    models_dir = Path(models_dir)
    models_dir.mkdir(parents=True, exist_ok=True)
    wanted = sorted({int(i) for i in file_ids})
    entries = {e["file_id"]: e for e in thingi10k.dataset(file_id=wanted)}
    missing = [i for i in wanted if i not in entries]
    if missing:
        raise SystemExit(f"error: no such models in the dataset: {missing}")
    skipped = 0
    for file_id in wanted:
        out = models_dir / f"{file_id}.off"
        if out.exists():
            continue
        vertices, facets = thingi10k.load_file(entries[file_id]["file_path"])
        try:
            text = _off_text(vertices, facets)
        except ValueError as error:
            # Not a triangle mesh the mesher can read: it gets no OFF file.
            print(f"skipped model {file_id}: {error}")
            skipped += 1
            continue
        partial = out.with_name(out.name + ".partial")
        partial.write_text(text, encoding="ascii")
        partial.replace(out)
    print(f"exported {len(wanted) - skipped} models to {models_dir}; skipped {skipped}")


def main(argv):
    if len(argv) == 3 and argv[0] == "metadata":
        write_metadata(argv[1], argv[2])
    elif len(argv) >= 3 and argv[0] == "export":
        export(argv[1], argv[2], argv[3:])
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
