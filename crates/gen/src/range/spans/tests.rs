use super::{Arena, MAX_COORD_COMPONENTS, MAX_OPERATIONS};
use crate::differences::difference_graph;
use crate::dyadic::Dyadic;
use crate::parse::parse;
use crate::range::shape::{difference_ops, raw_ops};

const ORIENT3D: &str = include_str!("../../../tests/fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("../../../tests/fixtures/insphere.pred");
const INSPHERE_RAW: &str = include_str!("../../../tests/fixtures/insphere_raw.pred");

/// The exponents of the product domain of the arithmetic core.
const LOW: i64 = -480;
const HIGH: i64 = 480;

fn translated(source: &str) -> Arena {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
    Arena::new(difference_ops(&g), MAX_COORD_COMPONENTS, true)
}

fn untranslated(source: &str) -> Arena {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = p.exact.unwrap_or(p.main);
    Arena::new(raw_ops(&g).unwrap(), MAX_COORD_COMPONENTS, false)
}

fn cases() -> Vec<(&'static str, Arena)> {
    vec![
        ("orient3d translated", translated(ORIENT3D)),
        ("orient3d untranslated", untranslated(ORIENT3D)),
        ("insphere translated", translated(INSPHERE)),
        ("insphere untranslated", untranslated(INSPHERE_RAW)),
    ]
}

fn exponent(power_of_two: f64) -> i64 {
    i64::from(power_of_two.log2() as i32)
}

#[test]
fn every_condition_holds_at_the_edges_and_fails_one_step_out() {
    for (name, arena) in cases() {
        let plan = arena.plan(LOW, HIGH).unwrap();
        let (m, big_m) = (exponent(plan.range.min), exponent(plan.range.max));
        assert!(arena.grids_fit(m, LOW), "{name}");
        assert!(!arena.grids_fit(m - 1, LOW), "{name}");
        assert!(arena.fit(m, big_m, HIGH).is_some(), "{name}");
        assert!(arena.fit(m, big_m + 1, HIGH).is_none(), "{name}");
    }
}

#[test]
fn the_caps_never_exceed_the_structural_lengths() {
    // With one component per coordinate and no cap, the lengths are those
    // of the stack model; with more, each length is at most its structural
    // one, computed here without the cap.
    for (name, arena) in cases() {
        let plan = arena.plan(LOW, HIGH).unwrap();
        let mut structural: Vec<u128> = Vec::new();
        for op in &arena.ops {
            use crate::range::shape::Op;
            let leaf = if arena.translated {
                2 * arena.components
            } else {
                arena.components
            };
            structural.push(match *op {
                Op::Leaf => leaf,
                Op::Constant(c) => u128::from(c != 0),
                Op::Neg(x) => structural[x],
                Op::Sum(x, y) => structural[x] + structural[y],
                Op::Product(x, y) => 2 * structural[x] * structural[y],
            });
        }
        for (length, bound) in plan.lengths.iter().zip(&structural) {
            assert!(u128::try_from(*length).unwrap() <= *bound, "{name}");
        }
        // The cap binds somewhere in every one of these forms.
        assert!(
            plan.lengths
                .iter()
                .zip(&structural)
                .any(|(&l, &s)| u128::try_from(l).unwrap() < s),
            "{name}"
        );
    }
}

#[test]
fn the_operation_counts_stay_far_below_their_limit() {
    // The factor `1 + 6 u T` of the bounds stays below `1 + 2^-20`.
    for (name, arena) in cases() {
        let plan = arena.plan(LOW, HIGH).unwrap();
        let lengths: Vec<u128> = plan.lengths.iter().map(|&l| l as u128).collect();
        assert!(lengths.iter().all(|&l| l < 1 << 20), "{name}: {lengths:?}");
    }
}

#[test]
fn the_operation_counts_follow_the_evaluation() {
    // `x * y` for two leaves of the translated form: each leaf merges two
    // coordinates of 16 components, `2 (16 + 16) = 64` transformations; the
    // product scales the 32 components of one leaf by each of the 32 of the
    // other, `3 * 32` each, and merges each scaling of 64 components into a
    // running sum of at most `l` components, `2 (l + 64)` each.
    let source = "fn p(a: Point3, b: Point3) -> Sign { (a.x - b.x) * (a.y - b.y) }";
    let arena = translated(source);
    let plan = arena.plan(LOW, HIGH).unwrap();
    let (m, big_m) = (exponent(plan.range.min), exponent(plan.range.max));
    let (lengths, counts) = arena.fit_with_counts(m, big_m, HIGH).unwrap();
    let l = lengths[2];
    assert_eq!(l, u128::try_from(plan.lengths[2]).unwrap());
    assert_eq!(counts, [64, 64, 64 + 64 + 32 * (3 * 32 + 2 * (l + 64))]);
    // Untranslated: the coordinates are read as they are, with no count;
    // `0 - x` is a merge from the empty expansion, `2 l_x`.
    let arena = untranslated("fn p(a: Point3, b: Point3) -> Sign { 0 - a.x * b.y }");
    let plan = arena.plan(LOW, HIGH).unwrap();
    let (m, big_m) = (exponent(plan.range.min), exponent(plan.range.max));
    let (lengths, counts) = arena.fit_with_counts(m, big_m, HIGH).unwrap();
    let product = 16 * (3 * 16 + 2 * (lengths[3] + 32));
    assert_eq!(counts, [0, 0, 0, product, product + 2 * lengths[3]]);
}

/// `d = a.x - b.x`, `q0 = d * d` and `q_k = q_(k-1) + q_(k-1)` up to
/// `q_n`: each doubling counts its operand twice, so the count of the root
/// doubles with every level.
fn doublings(n: usize) -> String {
    let mut body = String::from("let q0 = (a.x - b.x) * (a.x - b.x); ");
    for k in 1..=n {
        body += &format!("let q{k} = q{} + q{}; ", k - 1, k - 1);
    }
    format!("fn p(a: Point3, b: Point3) -> Sign {{ {body}q{n} }}")
}

#[test]
fn the_operation_limit_keeps_the_rounding_factor_below_seven_quarters() {
    // `6 u T <= 3/4` with `u = 2^-53`, so `B = (1 + 6 u T) S < 2 S`: the
    // step that caps the lengths.
    let six_u_t = &Dyadic::new(6, -53) * &Dyadic::integer(MAX_OPERATIONS);
    assert!(six_u_t <= Dyadic::new(3, -2));
    assert_eq!(MAX_OPERATIONS, 1 << 50);
}

#[test]
fn a_plan_whose_count_passes_the_limit_is_narrowed() {
    // With 32 doublings the count at the top of the range is within the
    // limit; with 33 it is above it, so the range must shrink until the
    // caps, and with them the count, fit.
    let at = |n: usize, big_m: i64| translated(&doublings(n)).fit_with_counts(-428, big_m, HIGH);
    let (_, counts) = at(32, 474).expect("32 doublings fit at 2^474");
    assert!(*counts.last().unwrap() <= MAX_OPERATIONS);
    assert!(*counts.last().unwrap() > MAX_OPERATIONS / 2);
    let plan = translated(&doublings(32)).plan(LOW, HIGH).unwrap();
    assert_eq!(exponent(plan.range.max), 474);
    assert!(
        at(33, 474).is_none(),
        "the count of 33 doublings exceeds the limit at 2^474"
    );
    match translated(&doublings(33)).plan(LOW, HIGH) {
        Ok(plan) => assert!(exponent(plan.range.max) < 474, "{plan:?}"),
        Err(e) => assert!(e.contains("no input range"), "{e}"),
    }
}
