"""Stratified, reproducible selection of the sample models.

Models are grouped into strata by vertex count tier, whether the mesh is
closed and whether it self-intersects. The target size is shared among the
strata in proportion to their size, with a minimum per stratum. Inside each
stratum the candidates are visited in a pseudo-random order fixed by the
seed, skipping those that are not valid. Validity is decided by a callback, so
the selection itself is a pure function of the metadata, the seed and the
callback's answers.
"""

import csv
import random
from dataclasses import dataclass
from fractions import Fraction

SEED = 20261110
TARGET = 200
MINIMUM_PER_STRATUM = 5

# Upper bounds (inclusive) of the vertex count tiers and their names; the last
# tier has no bound.
TIERS = [(1_000, "1k"), (10_000, "10k"), (100_000, "100k"), (None, "large")]

MANIFEST_COLUMNS = ["file_id", "stratum", "license", "vertices", "facets"]


@dataclass(frozen=True)
class Model:
    """The metadata of one dataset model that the selection needs."""

    file_id: int
    license: str
    vertices: int
    facets: int
    closed: bool
    self_intersecting: bool

    @property
    def stratum(self):
        tier = next(name for bound, name in TIERS if bound is None or self.vertices <= bound)
        closed = "closed" if self.closed else "open"
        si = "si" if self.self_intersecting else "nosi"
        return f"{tier}-{closed}-{si}"


@dataclass(frozen=True)
class Selection:
    """The selected models (by file id) and the candidates found invalid."""

    models: list
    invalid: list


def read_metadata(path):
    """Read the metadata CSV written by ``dataset.py metadata``."""
    with open(path, newline="", encoding="utf-8") as f:
        return [
            Model(
                file_id=int(row["file_id"]),
                license=row["license"],
                vertices=int(row["num_vertices"]),
                facets=int(row["num_facets"]),
                closed=row["closed"] == "1",
                self_intersecting=row["self_intersecting"] == "1",
            )
            for row in csv.DictReader(f)
        ]


def candidates(models, seed):
    """The candidates of every stratum, in their fixed pseudo-random order."""
    strata = {}
    for m in sorted(models, key=lambda m: m.file_id):
        strata.setdefault(m.stratum, []).append(m)
    for name, members in strata.items():
        random.Random(f"{seed}:{name}").shuffle(members)
    return strata


def allocate(weights, caps, target, minimum=MINIMUM_PER_STRATUM, least=None):
    """Share ``target`` among strata in proportion to ``weights``.

    No stratum gets more than its cap; each gets at least ``minimum`` (or its
    whole cap, when smaller), unless those minima alone exceed the target,
    and at least its entry in ``least`` (which must not exceed its cap).
    Rounding uses the largest remainder, with ties broken by stratum name.
    """
    names = sorted(weights)
    least = least or {}
    if sum(caps[n] for n in names) <= target:
        return {n: caps[n] for n in names}
    floors = {n: min(minimum, caps[n]) for n in names}
    if sum(floors.values()) > target:
        floors = {n: 0 for n in names}
    floors = {n: max(floors[n], least.get(n, 0)) for n in names}
    fixed = {}
    shares = {}
    while True:
        free = [n for n in names if n not in fixed]
        if not free:
            break
        remaining = target - sum(fixed.values())
        total = sum(weights[n] for n in free)
        shares = {n: Fraction(remaining * weights[n], total) for n in free}
        bound = {n: floors[n] for n in free if shares[n] < floors[n]}
        bound |= {n: caps[n] for n in free if shares[n] > caps[n]}
        if not bound:
            break
        fixed |= bound
    alloc = dict(fixed)
    alloc |= {n: int(shares[n]) for n in free}
    left = target - sum(alloc.values())
    by_remainder = sorted(free, key=lambda n: (-(shares[n] - int(shares[n])), n))
    for n in by_remainder[:left]:
        alloc[n] += 1
    return alloc


def select(models, target, seed, valid, lookahead=0):
    """Select ``target`` models, asking ``valid`` which candidates are valid.

    ``valid`` receives a list of models and returns the set of the file ids
    that are valid. It is asked in batches; with no ``lookahead`` it is asked
    about each model at most once. A stratum that runs out of valid candidates
    keeps those it has and its unfilled share goes to the other strata.

    A ``lookahead`` adds that many further candidates of each stratum still
    short of its share to every batch, so that a caller can check them in
    parallel; the candidates past the share are asked about again later, and
    the selection is the same with any lookahead.
    """
    order = candidates(models, seed)
    weights = {n: len(c) for n, c in order.items()}
    caps = dict(weights)
    position = {n: 0 for n in order}
    chosen = {n: [] for n in order}
    invalid = []
    while True:
        # Never take back a stratum's valid models: their count is a lower bound.
        alloc = allocate(weights, caps, target, least={n: len(c) for n, c in chosen.items()})
        need = {n: alloc[n] - len(chosen[n]) for n in order}
        batch = []
        for n, members in order.items():
            if need[n] > 0:
                batch += members[position[n]:position[n] + need[n] + lookahead]
        if not batch:
            break
        answer = valid(batch)
        # Take the answers in candidate order, up to each stratum's need.
        for m in batch:
            n = m.stratum
            if need[n] == 0:
                continue
            position[n] += 1
            if m.file_id in answer:
                chosen[n].append(m)
                need[n] -= 1
            else:
                invalid.append(m)
        for n, members in order.items():
            if position[n] >= len(members):
                caps[n] = len(chosen[n])
    models = sorted((m for c in chosen.values() for m in c), key=lambda m: m.file_id)
    return Selection(models, sorted(m.file_id for m in invalid))


def write_manifest(path, models):
    """Write the manifest: identifiers and metadata only, sorted by file id."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, lineterminator="\n")
        writer.writerow(MANIFEST_COLUMNS)
        for m in sorted(models, key=lambda m: m.file_id):
            writer.writerow([m.file_id, m.stratum, m.license, m.vertices, m.facets])


def read_manifest(path):
    """Read the manifest rows, with ``file_id`` as an integer."""
    with open(path, newline="", encoding="utf-8") as f:
        return [dict(row, file_id=int(row["file_id"])) for row in csv.DictReader(f)]
