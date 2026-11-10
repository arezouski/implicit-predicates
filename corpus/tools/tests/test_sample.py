"""Tests for the stratified selection of the sample models, on synthetic metadata."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import sample  # noqa: E402


def model(file_id, vertices=500, closed=True, self_intersecting=False):
    return sample.Model(
        file_id=file_id,
        license="Creative Commons - Attribution",
        vertices=vertices,
        facets=2 * vertices,
        closed=closed,
        self_intersecting=self_intersecting,
    )


def population():
    """Four strata of sizes 60, 30, 8 and 2."""
    models = []
    next_id = iter(range(1, 1000))
    for count, vertices, closed in [(60, 500, True), (30, 5000, True),
                                    (8, 500, False), (2, 50000, False)]:
        models += [model(next(next_id), vertices, closed) for _ in range(count)]
    return models


def all_valid(models):
    return {m.file_id for m in models}


class StratumTests(unittest.TestCase):
    def test_strata_cross_size_tier_closed_and_self_intersecting(self):
        self.assertEqual(model(1, 1000).stratum, "1k-closed-nosi")
        self.assertEqual(model(1, 1001, False).stratum, "10k-open-nosi")
        self.assertEqual(model(1, 100000, True, True).stratum, "100k-closed-si")
        self.assertEqual(model(1, 100001, False, True).stratum, "large-open-si")


class AllocationTests(unittest.TestCase):
    def test_allocation_sums_to_the_target(self):
        sizes = {"a": 600, "b": 333, "c": 41, "d": 26}
        for target in [10, 40, 100, 200, 999]:
            alloc = sample.allocate(sizes, sizes, target)
            self.assertEqual(sum(alloc.values()), target)
            for name, size in sizes.items():
                self.assertLessEqual(alloc[name], size)
                if target >= 5 * len(sizes):
                    self.assertGreaterEqual(alloc[name], min(5, size))

    def test_allocation_is_proportional_with_a_minimum(self):
        alloc = sample.allocate({"a": 900, "b": 90, "c": 10}, {"a": 900, "b": 90, "c": 10}, 100)
        self.assertEqual(alloc, {"a": 86, "b": 9, "c": 5})

    def test_small_strata_are_taken_whole(self):
        sizes = {"a": 1000, "b": 3, "c": 1}
        alloc = sample.allocate(sizes, sizes, 50)
        self.assertEqual(alloc, {"a": 46, "b": 3, "c": 1})
        chosen = sample.select(population(), 40, 7, all_valid).models
        by_stratum = {}
        for m in chosen:
            by_stratum.setdefault(m.stratum, []).append(m.file_id)
        self.assertEqual(sorted(by_stratum["100k-open-nosi"]), [99, 100])
        self.assertEqual(len(by_stratum["1k-open-nosi"]), 5)

    def test_a_population_smaller_than_the_target_is_taken_whole(self):
        sizes = {"a": 3, "b": 4}
        self.assertEqual(sample.allocate(sizes, sizes, 50), {"a": 3, "b": 4})


class SelectionTests(unittest.TestCase):
    def test_selection_is_deterministic(self):
        first = sample.select(population(), 40, 20261110, all_valid)
        second = sample.select(list(reversed(population())), 40, 20261110, all_valid)
        self.assertEqual(first, second)
        self.assertEqual(len(first.models), 40)
        other = sample.select(population(), 40, 1, all_valid)
        self.assertNotEqual(first.models, other.models)

    def test_invalid_candidates_are_replaced_within_the_stratum(self):
        seed = 20261110
        chosen = sample.select(population(), 40, seed, all_valid).models
        rejected = {m.file_id for m in chosen if m.stratum == "1k-closed-nosi"}
        rejected = set(sorted(rejected)[:3])

        def valid(models):
            return {m.file_id for m in models} - rejected

        replaced = sample.select(population(), 40, seed, valid)
        self.assertEqual(len(replaced.models), 40)
        self.assertEqual(replaced.invalid, sorted(rejected))
        old = {m.file_id: m for m in chosen}
        new = {m.file_id: m for m in replaced.models}
        self.assertFalse(rejected & new.keys())
        added = [new[i] for i in new.keys() - old.keys()]
        self.assertEqual(len(added), 3)
        self.assertTrue(all(m.stratum == "1k-closed-nosi" for m in added))
        order = [m.file_id for m in sample.candidates(population(), seed)["1k-closed-nosi"]]
        kept = [i for i in order if i in new and new[i].stratum == "1k-closed-nosi"]
        self.assertEqual(kept, [i for i in order if i not in rejected][:len(kept)])

    def test_an_exhausted_stratum_gives_its_share_to_the_others(self):
        def valid(models):
            return {m.file_id for m in models if m.stratum != "1k-open-nosi" or m.file_id < 93}

        result = sample.select(population(), 40, 3, valid)
        self.assertEqual(len(result.models), 40)
        self.assertEqual(sum(m.stratum == "1k-open-nosi" for m in result.models), 2)

    def test_validity_is_asked_once_per_model_in_batches(self):
        asked = []

        def valid(models):
            asked.extend(m.file_id for m in models)
            return {m.file_id for m in models if m.file_id % 3}

        sample.select(population(), 40, 5, valid)
        self.assertEqual(len(asked), len(set(asked)))


    def test_the_lookahead_does_not_change_the_selection(self):
        def valid(models):
            return {m.file_id for m in models if m.file_id % 3}

        plain = sample.select(population(), 40, 9, valid)
        for lookahead in [1, 4, 100]:
            self.assertEqual(sample.select(population(), 40, 9, valid, lookahead), plain)


class ManifestTests(unittest.TestCase):
    def test_manifest_round_trip(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "sample.csv"
            models = [model(3, 20), model(1, 2000, False, True)]
            sample.write_manifest(path, models)
            text = path.read_text()
            self.assertTrue(text.startswith("file_id,stratum,license,vertices,facets\n"))
            self.assertEqual([r["file_id"] for r in sample.read_manifest(path)], [1, 3])

    def test_metadata_is_read_from_the_dataset_csv(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "metadata.csv"
            path.write_text(
                "file_id,thing_id,author,license,num_vertices,num_facets,closed,self_intersecting\n"
                "42,7,someone,Public Domain,10,16,1,0\n"
            )
            [m] = sample.read_metadata(path)
            self.assertEqual(m, sample.Model(42, "Public Domain", 10, 16, True, False))


if __name__ == "__main__":
    unittest.main()
