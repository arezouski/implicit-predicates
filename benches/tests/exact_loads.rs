//! Checks the loads of the exact stages of `dihedral_sign`, `dot_sign_3d`
//! and `orient3d` on implicit points against the exact signs of the
//! hand-written oracle (`implicit_predicates_corpus::exact` on the
//! definitions of the points), and prints, in an ignored test, the
//! peak arena use of each stage on them.
//!
//! The peak arena use is deterministic, so it is measured here, not in the
//! benchmark: `cargo test -p implicit-predicates-benches --release --test
//! exact_loads -- --ignored --nocapture`.

use implicit_predicates::{AnyPoint, Point, Scratch, Sign};
use implicit_predicates_benches::predicates::{
    DIHEDRAL_SIGN, DOT_SIGN_3D, EXACT_LOAD_SIZE, ExactLoad, ExactStages, ORIENT3D,
    dihedral_sign_loads, dot_sign_3d_loads, orient3d_exact_loads,
};

/// The number of points of each type of a call: explicit, `Lnc`, `Bpt`.
fn types<const N: usize>(case: &[AnyPoint; N]) -> [usize; 3] {
    let mut out = [0; 3];
    for point in case {
        match point {
            AnyPoint::Explicit(_) => out[0] += 1,
            AnyPoint::Lnc(_) => out[1] += 1,
            AnyPoint::Bpt(_) => out[2] += 1,
            other => panic!("unexpected point {other:?}"),
        }
    }
    out
}

/// The number of points of each type a load of this name has.
fn expected_types(name: &str) -> [usize; 3] {
    match name.split('/').next().expect("a kind") {
        "4_lnc" => [0, 4, 0],
        "4_bpt" => [0, 0, 4],
        "3_lnc" => [0, 3, 0],
        "3_bpt" => [0, 0, 3],
        "mixed" => [1, 2, 1],
        other => panic!("no kind {other}"),
    }
}

/// Checks every call of every load: its point types, that the filters do
/// not decide it, its exact sign against the family, and that every stage
/// that decides it has the exact sign.
fn check<const N: usize>(stages: &ExactStages<N>, loads: &[ExactLoad<N>]) {
    let mut scratch = Scratch::growable(1 << 16);
    for load in loads {
        let what = format!("{} {}", stages.name, load.name);
        if load.cases.len() < EXACT_LOAD_SIZE {
            assert!(load.name.contains("mesh"), "{what}");
            println!(
                "{what}: only {} calls of {} tried",
                load.cases.len(),
                load.tried
            );
        }
        assert_eq!(
            load.degenerate,
            load.name.ends_with("/degenerate"),
            "{what}"
        );
        let mut decided_by_expansions = 0;
        for case in &load.cases {
            assert_eq!(types(case), expected_types(&load.name), "{what}");
            assert_eq!((stages.filters)(case), None, "{what}: {case:?}");
            let sign = (stages.oracle)(case);
            if load.degenerate {
                assert_eq!(sign, 0, "{what}: {case:?}");
            } else {
                assert_ne!(sign, 0, "{what}: {case:?}");
            }
            let big = stages.big_integers(&mut scratch, case);
            assert_eq!(i8::from(big), sign, "{what} big integers: {case:?}");
            let chain = stages.chain(&mut scratch, case);
            assert_eq!(i8::from(chain), sign, "{what} chain: {case:?}");
            if let Some(s) = stages.expansions(&mut scratch, case) {
                assert_eq!(i8::from(s), sign, "{what} expansions: {case:?}");
                decided_by_expansions += 1;
            }
            assert_eq!(scratch.used(), 0, "{what}");
        }
        // The stage on expansions is measured on these loads, so it must
        // decide them: every point is in its range and short enough.
        assert_eq!(decided_by_expansions, load.cases.len(), "{what}");
    }
}

#[test]
fn the_dihedral_sign_loads_have_the_exact_sign_and_are_left_to_the_exact_stages() {
    let loads = dihedral_sign_loads();
    assert_eq!(loads.len(), 30);
    check(&DIHEDRAL_SIGN, &loads);
}

#[test]
fn the_dot_sign_3d_loads_have_the_exact_sign_and_are_left_to_the_exact_stages() {
    let loads = dot_sign_3d_loads();
    assert_eq!(loads.len(), 20);
    check(&DOT_SIGN_3D, &loads);
}

#[test]
fn the_orient3d_loads_have_the_exact_sign_and_are_left_to_the_exact_stages() {
    let loads = orient3d_exact_loads();
    assert_eq!(loads.len(), 20);
    check(&ORIENT3D, &loads);
}

#[test]
fn the_loads_are_deterministic() {
    assert_eq!(
        format!("{:?}", dihedral_sign_loads()),
        format!("{:?}", dihedral_sign_loads())
    );
    assert_eq!(
        format!("{:?}", dot_sign_3d_loads()),
        format!("{:?}", dot_sign_3d_loads())
    );
    assert_eq!(
        format!("{:?}", orient3d_exact_loads()),
        format!("{:?}", orient3d_exact_loads())
    );
}

/// The median, 99th percentile and maximum of `values`.
fn summary(mut values: Vec<usize>) -> (usize, usize, usize) {
    values.sort_unstable();
    let at = |q: f64| values[((values.len() - 1) as f64 * q).round() as usize];
    (at(0.5), at(0.99), at(1.0))
}

/// The peak arena use of `f` on a fresh growable arena, in `f64`.
fn peak(f: impl FnOnce(&mut Scratch<'_>)) -> usize {
    let mut scratch = Scratch::growable(0);
    f(&mut scratch);
    scratch.high_water()
}

/// Prints one row of the table for each load.
fn table<const N: usize>(stages: &ExactStages<N>, loads: &[ExactLoad<N>]) {
    for load in loads {
        let mut coords = Vec::new();
        let mut expansions = Vec::new();
        let mut big = Vec::new();
        let mut chain = Vec::new();
        let (mut declined, mut long) = (0, 0);
        let mut longest = 0;
        let mut decided: Option<Sign> = None;
        for case in &load.cases {
            coords.push(peak(|s| {
                let c = stages.exact_coords(s, case).expect("exact coordinates");
                let len = longest_coordinate(&c);
                longest = longest.max(len);
                if len > stages.max_coord_components {
                    long += 1;
                }
            }));
            expansions.push(peak(|s| {
                decided = stages.expansions(s, case);
            }));
            if decided.is_none() {
                declined += 1;
            }
            big.push(peak(|s| {
                stages.big_integers(s, case);
            }));
            chain.push(peak(|s| {
                stages.chain(s, case);
            }));
        }
        if load.cases.is_empty() {
            println!(
                "{:<14} {:<32} empty: the filters decide all {} calls tried",
                stages.name, load.name, load.tried
            );
            continue;
        }
        let fmt = |(m, p, x): (usize, usize, usize)| format!("{m:>6} {p:>6} {x:>6}");
        let st = Stats::of(load.cases.iter());
        println!(
            "{:<14} {:<32} {} | {} | {} | {} | {:>4} {:>4} {:>3} | kept {}/{} | {}",
            stages.name,
            load.name,
            fmt(summary(coords)),
            fmt(summary(expansions)),
            fmt(summary(big)),
            fmt(summary(chain)),
            declined,
            long,
            longest,
            load.cases.len(),
            load.tried,
            st.line(),
        );
    }
}

/// The largest number of components of a coordinate of `coords`.
fn longest_coordinate<const N: usize>(coords: &[implicit_predicates::ExactCoords; N]) -> usize {
    coords
        .iter()
        .flat_map(|q| [q.x, q.y, q.z])
        .map(|s| s.len())
        .max()
        .unwrap_or(0)
}

#[test]
#[ignore = "prints a table; run with --ignored --nocapture"]
fn peak_arena_use_of_the_exact_stages() {
    println!(
        "peak arena use in f64 (median, 99th percentile, maximum) of: the exact coordinates \
         alone | the stage on expansions with them | the stage on big integers with its \
         coordinates | the exact chain; then the calls the stage on expansions declines, \
         those with a coordinate longer than MAX_COORD_COMPONENTS, and the longest \
         coordinate, in components; {EXACT_LOAD_SIZE} calls per load"
    );
    table(&DIHEDRAL_SIGN, &dihedral_sign_loads());
    table(&DOT_SIGN_3D, &dot_sign_3d_loads());
    table(&ORIENT3D, &orient3d_exact_loads());
}

/// Statistics of the calls of a set: the number of components of every
/// exact coordinate of every implicit point, the calls in which two
/// arguments share a parent (by value) or are equal, the parameters that
/// are dyadic numbers of at most 8 significant bits, and the implicit
/// points two of whose parents share a coordinate value on one axis.
#[derive(Default)]
struct Stats {
    calls: usize,
    components: Vec<usize>,
    sharing: usize,
    params: usize,
    short_params: usize,
    implicit: usize,
    axis_sharing: usize,
}

/// The parents of a point: the point itself when explicit.
fn parents(p: &AnyPoint) -> Vec<[f64; 3]> {
    match p {
        AnyPoint::Explicit(e) => vec![*e.approx()],
        AnyPoint::Lnc(l) => vec![*l.p(), *l.q()],
        AnyPoint::Bpt(b) => vec![*b.p(), *b.q(), *b.r()],
        other => panic!("unexpected point {other:?}"),
    }
}

/// The parameters of a point.
fn params(p: &AnyPoint) -> Vec<f64> {
    match p {
        AnyPoint::Explicit(_) => vec![],
        AnyPoint::Lnc(l) => vec![l.t()],
        AnyPoint::Bpt(b) => vec![b.u(), b.v()],
        other => panic!("unexpected point {other:?}"),
    }
}

/// The number of significant bits of `x`, 0 for zero.
fn significant_bits(x: f64) -> u32 {
    if x == 0.0 {
        return 0;
    }
    let bits = x.to_bits() & ((1u64 << 52) - 1) | (1u64 << 52);
    53 - bits.trailing_zeros()
}

impl Stats {
    fn of<'a, const N: usize>(cases: impl Iterator<Item = &'a [AnyPoint; N]>) -> Stats {
        let mut st = Stats::default();
        let mut scratch = Scratch::growable(1 << 12);
        for case in cases {
            st.calls += 1;
            let ps: Vec<Vec<[f64; 3]>> = case.iter().map(parents).collect();
            let shares = (0..N).any(|i| {
                (i + 1..N).any(|j| case[i] == case[j] || ps[i].iter().any(|a| ps[j].contains(a)))
            });
            st.sharing += usize::from(shares);
            for point in case {
                for t in params(point) {
                    st.params += 1;
                    st.short_params += usize::from(significant_bits(t) <= 8);
                }
                if point.is_explicit() {
                    continue;
                }
                st.implicit += 1;
                let pp = parents(point);
                let axis = (0..3).any(|k| {
                    (0..pp.len()).any(|i| (i + 1..pp.len()).any(|j| pp[i][k] == pp[j][k]))
                });
                st.axis_sharing += usize::from(axis);
                let mark = scratch.mark();
                let q = point.exact(&mut scratch).expect("exact coordinates");
                st.components.extend([q.x.len(), q.y.len(), q.z.len()]);
                scratch.reset(mark);
            }
        }
        st
    }

    fn line(&self) -> String {
        let pct = |a: usize, b: usize| {
            if b == 0 {
                0.0
            } else {
                100.0 * a as f64 / b as f64
            }
        };
        let (m, p, x) = summary(self.components.clone());
        let mut hist = [0usize; 10];
        for &c in &self.components {
            hist[c.min(9)] += 1;
        }
        format!(
            "comp {m}/{p}/{x} hist(0..9+) {hist:?} | share {:.0}% | short params {:.0}% | axis {:.0}%",
            pct(self.sharing, self.calls),
            pct(self.short_params, self.params),
            pct(self.axis_sharing, self.implicit),
        )
    }
}

#[allow(dead_code)]
#[path = "../../crates/implicit-predicates/tests/common/corpus_points.rs"]
mod corpus_points;

#[test]
#[ignore = "prints statistics; run with --ignored --nocapture"]
fn statistics_of_the_recorded_orient3d_calls() {
    use implicit_predicates::stages::orient3d;
    use implicit_predicates_corpus::{Corpus, Predicate};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/samples");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("the samples")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "ipcorpus"))
        .collect();
    files.sort();
    let mut by_count: Vec<Vec<[AnyPoint; 4]>> = vec![Vec::new(); 5];
    let mut all_stored = 0;
    for path in &files {
        let corpus = Corpus::read(std::io::BufReader::new(
            std::fs::File::open(path).expect("open"),
        ))
        .expect("a valid corpus");
        let mapped = corpus_points::points(&corpus);
        for call in &corpus.calls {
            if call.predicate != Predicate::Orient3d {
                continue;
            }
            let k = corpus_points::implicit_count(&corpus, call);
            if k == 0 {
                continue;
            }
            all_stored += 1;
            let Some(p) = corpus_points::call_points::<4>(&mapped, call) else {
                continue;
            };
            let [a, b, c, d] = &p;
            if orient3d::filter_points(a, b, c, d).is_none() {
                by_count[k].push(p);
            }
        }
    }
    println!(
        "{} files, {all_stored} stored orient3d calls with implicit points (unweighted)",
        files.len()
    );
    for (k, calls) in by_count.iter().enumerate().skip(1) {
        println!(
            "recorded orient3d {k}_implicit undecided by the filter: {} calls | {}",
            calls.len(),
            Stats::of(calls.iter()).line()
        );
    }
    let all: Vec<_> = by_count.iter().flatten().copied().collect();
    println!(
        "recorded orient3d all undecided: {} calls | {}",
        all.len(),
        Stats::of(all.iter()).line()
    );
}
