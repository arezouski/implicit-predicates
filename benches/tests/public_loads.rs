//! Checks the loads of the public predicates (`dot_sign_3d`,
//! `dihedral_sign`, `orient3d` and `insphere` with `Bpt`) against the exact
//! signs of the hand-written oracle: the point types of each load, the sign
//! each family claims, the decision of the filter, and every contender and
//! stage that decides. An ignored test prints which stage decides the calls
//! of each load:
//!
//! `cargo test -p implicit-predicates-benches --release --test public_loads
//! -- --ignored --nocapture stages_that_decide`.

use implicit_predicates::stages::{dihedral_sign, dot_sign_3d, insphere, orient3d};
use implicit_predicates::{AnyPoint, Point, Scratch, Sign};
use implicit_predicates_benches::predicates::{
    DIHEDRAL_SIGN, DOT_SIGN_3D, ExactStages, Family, ORIENT3D, PublicLoad, SET_SIZE,
    dihedral_sign_public_loads, dot_sign_3d_public_loads, exact_dihedral_sign, exact_dot_sign_3d,
    exact_insphere_sign, exact_orient3d_sign, implicit_predicates_dihedral_sign_sign,
    implicit_predicates_dot_sign_3d_sign, implicit_predicates_insphere_any_sign,
    implicit_predicates_insphere_public_sign, implicit_predicates_orient3d_any_sign,
    insphere_big_integers_stage, insphere_bpt_loads, naive_dihedral_sign_sign,
    naive_dot_sign_3d_sign, orient3d_bpt_loads,
};

/// The stage that decides a call of the public chain, in the order the
/// chain tries them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// `filter_points`, the first test of the filter.
    Filter,
    /// `filter_axes_points`, the second test.
    FilterAxes,
    /// The exact stages on the stack, for explicit points.
    Stack,
    /// The stage on expansions over arena spans.
    Expansions,
    /// The stage on big integers.
    BigIntegers,
}

const STAGES: [Stage; 5] = [
    Stage::Filter,
    Stage::FilterAxes,
    Stage::Stack,
    Stage::Expansions,
    Stage::BigIntegers,
];

/// A stage that decides a call or declines.
type Test<const N: usize> = fn(&[AnyPoint; N]) -> Option<Sign>;

/// What a predicate runs, stage by stage, as a function of the points of a
/// call.
struct Chain<const N: usize> {
    name: &'static str,
    /// The names of the families, in the order of `Family`.
    families: [&'static str; 3],
    filter: Test<N>,
    filter_axes: Option<Test<N>>,
    /// The exact stages on the stack, which take the approximations of
    /// explicit points.
    stack: Option<Test<N>>,
    /// The stage on expansions over arena spans, if the predicate has one.
    expansions: Option<&'static ExactStages<N>>,
    /// The stage on big integers.
    big: fn(&mut Scratch<'_>, &[AnyPoint; N]) -> Sign,
    /// The exact sign.
    oracle: fn(&[AnyPoint; N]) -> i8,
    /// The public function.
    public: fn(&[AnyPoint; N]) -> i8,
}

/// The stage that decides `case`, with the sign it gives, after checking
/// that every stage that decides gives the exact sign.
fn deciding<const N: usize>(
    chain: &Chain<N>,
    scratch: &mut Scratch<'_>,
    case: &[AnyPoint; N],
    what: &str,
) -> Stage {
    let exact = (chain.oracle)(case);
    let all_explicit = case.iter().all(|p| p.is_explicit());
    let results = [
        (Stage::Filter, (chain.filter)(case)),
        (
            Stage::FilterAxes,
            chain.filter_axes.and_then(|stage| stage(case)),
        ),
        (
            Stage::Stack,
            chain.stack.filter(|_| all_explicit).and_then(|f| f(case)),
        ),
        (
            Stage::Expansions,
            chain.expansions.and_then(|s| s.expansions(scratch, case)),
        ),
        (Stage::BigIntegers, Some((chain.big)(scratch, case))),
    ];
    for (stage, sign) in results {
        if let Some(sign) = sign {
            assert_eq!(i8::from(sign), exact, "{what} {stage:?}: {case:?}");
        }
    }
    assert_eq!((chain.public)(case), exact, "{what} public: {case:?}");
    assert_eq!(scratch.used(), 0, "{what}");
    results
        .into_iter()
        .find_map(|(stage, sign)| sign.map(|_| stage))
        .expect("big integers decide every call")
}

const ANGLE_FAMILIES: [&str; 3] = ["random", "near_right", "right"];
const DEGENERATE_FAMILIES: [&str; 3] = ["random", "near_degenerate", "degenerate"];

const DIHEDRAL_CHAIN: Chain<4> = Chain {
    name: "dihedral_sign",
    families: ANGLE_FAMILIES,
    filter: |[a, b, c, d]| dihedral_sign::filter_points(a, b, c, d),
    filter_axes: Some(|[a, b, c, d]| dihedral_sign::filter_axes_points(a, b, c, d)),
    stack: Some(|[a, b, c, d]| dihedral_sign::exact_differences([a, b, c, d].map(|p| p.approx()))),
    expansions: Some(&DIHEDRAL_SIGN),
    big: |s, c| DIHEDRAL_SIGN.big_integers(s, c),
    oracle: exact_dihedral_sign,
    public: implicit_predicates_dihedral_sign_sign,
};

const DOT_CHAIN: Chain<3> = Chain {
    name: "dot_sign_3d",
    families: ANGLE_FAMILIES,
    filter: |[p, q, r]| dot_sign_3d::filter_points(p, q, r),
    filter_axes: Some(|[p, q, r]| dot_sign_3d::filter_axes_points(p, q, r)),
    stack: Some(|[p, q, r]| {
        let p = [p, q, r].map(|x| x.approx());
        dot_sign_3d::exact_differences(p).or_else(|| dot_sign_3d::expansions_translated(p))
    }),
    expansions: Some(&DOT_SIGN_3D),
    big: |s, c| DOT_SIGN_3D.big_integers(s, c),
    oracle: exact_dot_sign_3d,
    public: implicit_predicates_dot_sign_3d_sign,
};

const ORIENT3D_CHAIN: Chain<4> = Chain {
    name: "orient3d",
    families: DEGENERATE_FAMILIES,
    filter: |[a, b, c, d]| orient3d::filter_points(a, b, c, d),
    filter_axes: None,
    stack: None,
    expansions: Some(&ORIENT3D),
    big: |s, c| ORIENT3D.big_integers(s, c),
    oracle: exact_orient3d_sign,
    public: implicit_predicates_orient3d_any_sign,
};

const INSPHERE_CHAIN: Chain<5> = Chain {
    name: "insphere",
    families: DEGENERATE_FAMILIES,
    filter: |[a, b, c, d, e]| insphere::filter_points(a, b, c, d, e),
    filter_axes: Some(|[a, b, c, d, e]| insphere::filter_axes_points(a, b, c, d, e)),
    stack: None,
    expansions: None,
    big: insphere_big_integers_stage,
    oracle: exact_insphere_sign,
    public: implicit_predicates_insphere_any_sign,
};

/// The point types of a call, one letter each as the loads name them.
fn types<const N: usize>(case: &[AnyPoint; N]) -> String {
    case.iter()
        .map(|p| match p {
            AnyPoint::Explicit(_) => 'E',
            AnyPoint::Lnc(_) => 'L',
            AnyPoint::Bpt(_) => 'B',
            other => panic!("unexpected point {other:?}"),
        })
        .collect()
}

/// The point types a load of `predicate` of this name has.
fn expected_types(predicate: &str, name: &str, points: usize) -> String {
    match (predicate, name.split('/').next().expect("a kind")) {
        (_, "explicit") => "E".repeat(points),
        (_, kind) if kind.ends_with("_lnc") => "L".repeat(points),
        (_, kind) if kind.ends_with("_bpt") && kind != "mixed_bpt" => "B".repeat(points),
        ("dihedral_sign", "mixed") => "LBLE".to_string(),
        ("dot_sign_3d", "mixed") => "LLB".to_string(),
        ("insphere", "mixed_bpt") => "EEBBB".to_string(),
        other => panic!("no load {other:?}"),
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

/// The parameters of a point.
fn params(p: &AnyPoint) -> Vec<f64> {
    match p {
        AnyPoint::Explicit(_) => vec![],
        AnyPoint::Lnc(l) => vec![l.t()],
        AnyPoint::Bpt(b) => vec![b.u(), b.v()],
        other => panic!("unexpected point {other:?}"),
    }
}

/// The calls of every family of a load, the stage that decides each, and the
/// counts of each stage; checks every call.
fn check<const N: usize>(chain: &Chain<N>, loads: &[PublicLoad<N>]) -> Vec<[usize; 5]> {
    let mut scratch = Scratch::growable(1 << 16);
    let mut names = Vec::new();
    let mut table = Vec::new();
    for load in loads {
        let what = format!("{} {}", chain.name, load.name);
        assert!(!names.contains(&load.name), "{what}: a name used twice");
        names.push(load.name.clone());
        assert_eq!(load.cases.len(), SET_SIZE, "{what}");
        let family = load.name.rsplit('/').next().expect("a family");
        assert_eq!(family, chain.families[load.family as usize], "{what}");
        let mesh = load.name.contains("/mesh/");
        let mut counts = [0; 5];
        for case in &load.cases {
            assert_eq!(
                types(case),
                expected_types(chain.name, &load.name, N),
                "{what}"
            );
            let exact = (chain.oracle)(case);
            let stage = deciding(chain, &mut scratch, case, &what);
            counts[STAGES.iter().position(|&s| s == stage).expect("a stage")] += 1;
            match load.family {
                Family::Random => {
                    assert_ne!(exact, 0, "{what}: {case:?}");
                    assert_eq!(stage, Stage::Filter, "{what}: {case:?}");
                }
                Family::Near => {
                    assert_ne!(exact, 0, "{what}: {case:?}");
                    assert_ne!(stage, Stage::Filter, "{what}: {case:?}");
                }
                Family::Exact => {
                    assert_eq!(exact, 0, "{what}: {case:?}");
                    assert_ne!(stage, Stage::Filter, "{what}: {case:?}");
                }
            }
            // The parameters of a `mesh` load have at most 8 significant
            // bits; those of the others are not required to be long, but see
            // below.
            if mesh {
                for t in case.iter().flat_map(params) {
                    assert!(significant_bits(t) <= 8, "{what}: {t} in {case:?}");
                }
            }
        }
        if load.name.contains("/params53/") {
            let long = load
                .cases
                .iter()
                .flatten()
                .flat_map(params)
                .filter(|&t| significant_bits(t) > 8)
                .count();
            let all = load.cases.iter().flatten().flat_map(params).count();
            assert!(10 * long >= 9 * all, "{what}: {long} of {all}");
        }
        table.push(counts);
    }
    table
}

#[test]
fn the_dihedral_sign_loads_are_what_they_claim_and_every_stage_has_the_exact_sign() {
    let loads = dihedral_sign_public_loads();
    assert_eq!(loads.len(), 21);
    check(&DIHEDRAL_CHAIN, &loads);
    // The naive expression has the sign of the exact computation where the
    // filter decides, so it is a lower bound of the cost on the explicit
    // loads that checks the convention.
    for load in loads.iter().filter(|l| l.name.starts_with("explicit/")) {
        if load.family == Family::Random {
            for case in &load.cases {
                assert_eq!(naive_dihedral_sign_sign(case), exact_dihedral_sign(case));
            }
        }
    }
}

#[test]
fn the_dot_sign_3d_loads_are_what_they_claim_and_every_stage_has_the_exact_sign() {
    let loads = dot_sign_3d_public_loads();
    assert_eq!(loads.len(), 21);
    check(&DOT_CHAIN, &loads);
    for load in loads.iter().filter(|l| l.name.starts_with("explicit/")) {
        if load.family == Family::Random {
            for case in &load.cases {
                assert_eq!(naive_dot_sign_3d_sign(case), exact_dot_sign_3d(case));
            }
        }
    }
}

#[test]
fn the_orient3d_bpt_loads_are_what_they_claim_and_every_stage_has_the_exact_sign() {
    let loads = orient3d_bpt_loads();
    assert_eq!(loads.len(), 6);
    check(&ORIENT3D_CHAIN, &loads);
}

#[test]
fn the_insphere_bpt_loads_are_what_they_claim_and_every_stage_has_the_exact_sign() {
    let loads = insphere_bpt_loads();
    assert_eq!(loads.len(), 8);
    check(&INSPHERE_CHAIN, &loads);
    for load in &loads {
        for case in &load.cases {
            assert_eq!(
                implicit_predicates_insphere_public_sign(case),
                exact_insphere_sign(case),
                "{}: {case:?}",
                load.name
            );
        }
    }
}

#[test]
fn the_loads_are_deterministic() {
    assert_eq!(
        format!("{:?}", dihedral_sign_public_loads()),
        format!("{:?}", dihedral_sign_public_loads())
    );
    assert_eq!(
        format!("{:?}", dot_sign_3d_public_loads()),
        format!("{:?}", dot_sign_3d_public_loads())
    );
    assert_eq!(
        format!("{:?}", orient3d_bpt_loads()),
        format!("{:?}", orient3d_bpt_loads())
    );
    assert_eq!(
        format!("{:?}", insphere_bpt_loads()),
        format!("{:?}", insphere_bpt_loads())
    );
}

/// Prints the fraction of the calls of each load decided by each stage.
fn print_table<const N: usize>(chain: &Chain<N>, loads: &[PublicLoad<N>]) {
    let table = check(chain, loads);
    for (load, counts) in loads.iter().zip(table) {
        let pct = |n: usize| 100.0 * n as f64 / load.cases.len() as f64;
        println!(
            "{:<14} {:<36} {:>7.1} {:>7.1} {:>7.1} {:>7.1} {:>7.1}",
            chain.name,
            load.name,
            pct(counts[0]),
            pct(counts[1]),
            pct(counts[2]),
            pct(counts[3]),
            pct(counts[4]),
        );
    }
}

#[test]
#[ignore = "prints a table; run with --ignored --nocapture"]
fn stages_that_decide_the_public_loads() {
    println!(
        "percentage of the {SET_SIZE} calls of each load that each stage of the public chain decides, \
         the first that does; stack: the exact stages on the stack, explicit points only; \
         expans: the stage on expansions over arena spans, which insphere does not have"
    );
    println!(
        "{:<14} {:<36} {:>7} {:>7} {:>7} {:>7} {:>7}",
        "predicate", "load", "filter", "axes", "stack", "expans", "big"
    );
    print_table(&DIHEDRAL_CHAIN, &dihedral_sign_public_loads());
    print_table(&DOT_CHAIN, &dot_sign_3d_public_loads());
    print_table(&ORIENT3D_CHAIN, &orient3d_bpt_loads());
    print_table(&INSPHERE_CHAIN, &insphere_bpt_loads());
}
