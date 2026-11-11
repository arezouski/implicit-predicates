//! `orient3d` against the exact sign on every family of inputs, its
//! documented range, and the symmetries of the determinant.

mod common;

use common::generated::exact;
use common::{Points, STAGES, all_finite, all_in_range, edges, first_deciding, orient3d, refs};
use implicit_predicates::Sign;
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{
    config, coplanar4, extreme4, grid4, near_coplanar4, random4, repeated4, signed_zeros4,
};
use proptest::prelude::*;

/// One set of points of every family.
fn families() -> impl Strategy<Value = [(&'static str, Points); 7]> {
    // Boxed, so that the value tree of the tuple stays small on the stack.
    (
        random4().boxed(),
        coplanar4().boxed(),
        near_coplanar4().boxed(),
        grid4().boxed(),
        repeated4().boxed(),
        signed_zeros4().boxed(),
        extreme4(&edges()).boxed(),
    )
        .prop_map(|(r, c, n, g, rep, z, x)| {
            [
                ("random", r),
                ("coplanar", c),
                ("nearly coplanar", n),
                ("grid", g),
                ("repeated", rep),
                ("signed zeros", z),
                ("extreme", x),
            ]
        })
}

/// The four points permuted: `permuted(p, s)[i] = p[s[i]]`.
fn permuted(p: &Points, s: [usize; 4]) -> Points {
    s.map(|i| p[i])
}

/// The sign of a permutation of four elements.
fn is_even(s: [usize; 4]) -> bool {
    let inversions = (0..4)
        .flat_map(|i| (i + 1..4).map(move |j| (i, j)))
        .filter(|&(i, j)| s[i] > s[j])
        .count();
    inversions % 2 == 0
}

/// The largest power of two that divides the normal `x`, or `None` for
/// zero.
fn grid_unit(x: f64) -> Option<f64> {
    if x == 0.0 {
        return None;
    }
    assert!(x.is_normal());
    let bits = x.to_bits();
    let significand = (bits & ((1 << 52) - 1)) | (1 << 52);
    let biased = i32::try_from((bits >> 52) & 0x7ff).unwrap();
    let exponent = biased - 1075 + i32::try_from(significand.trailing_zeros()).unwrap();
    Some(2f64.powi(exponent))
}

/// Every permutation of four elements.
fn permutations() -> Vec<[usize; 4]> {
    let mut all = Vec::new();
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let s = [a, b, c, d];
                    if (0..4).all(|i| s.contains(&i)) {
                        all.push(s);
                    }
                }
            }
        }
    }
    all
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn orient3d_matches_the_exact_sign(families in families()) {
        for (family, p) in families {
            let expected = exact(&p);
            if family == "coplanar" || family == "repeated" {
                prop_assert_eq!(expected, Sign::Zero, "{} {:?}", family, p);
            }
            match orient3d(&p) {
                Some(sign) => prop_assert_eq!(sign, expected, "{} {:?}", family, p),
                None => prop_assert!(
                    !all_in_range(&p, stages::UNTRANSLATED_RANGE),
                    "{} {:?} is in the documented range",
                    family,
                    p
                ),
            }
        }
    }

    #[test]
    fn none_only_outside_the_documented_range(families in families()) {
        for (family, p) in families {
            // The chain ends with the two expansion stages, which decide
            // exactly when every coordinate is zero or has a magnitude in
            // their range; outside both only the filter and the exact
            // differences can decide.
            let translated = all_in_range(&p, stages::TRANSLATED_RANGE);
            let inside = all_in_range(&p, stages::UNTRANSLATED_RANGE);
            let earlier = stages::filter(refs(&p), 0.0)
                .or_else(|| stages::exact_differences(refs(&p)));
            let result = orient3d(&p);
            prop_assert_eq!(
                result.is_some(),
                earlier.is_some() || translated || inside,
                "{} {:?}",
                family,
                p
            );
            prop_assert_eq!(
                stages::expansions_translated(refs(&p)).is_some(),
                translated,
                "{} {:?}",
                family,
                p
            );
            prop_assert_eq!(
                stages::expansions_untranslated(refs(&p)).is_some(),
                inside,
                "{} {:?}",
                family,
                p
            );
            if result.is_none() {
                prop_assert!(!inside, "{} {:?}", family, p);
            }
            if inside {
                prop_assert!(all_finite(&p));
                prop_assert!(result.is_some(), "{} {:?}", family, p);
            }
        }
    }

    #[test]
    fn the_first_stage_that_decides_gives_the_result_of_the_chain(families in families()) {
        for (family, p) in families {
            prop_assert_eq!(
                first_deciding(&p).map(|(_, sign)| sign),
                orient3d(&p),
                "{} {:?}",
                family,
                p
            );
        }
    }

    #[test]
    fn the_generated_evaluator_matches_the_handwritten_one(families in families()) {
        for (family, p) in families {
            let [a, b, c, d] = p.map(|point| point.map(Dyadic::from_f64));
            let handwritten = match implicit_predicates_corpus::exact::orient3d(&a, &b, &c, &d) {
                -1 => Sign::Negative,
                0 => Sign::Zero,
                1 => Sign::Positive,
                other => panic!("not a sign: {other}"),
            };
            prop_assert_eq!(exact(&p), handwritten, "{} {:?}", family, p);
        }
    }

    #[test]
    fn swapping_two_arguments_negates(families in families()) {
        for (family, p) in families {
            let s = orient3d(&p);
            for i in 0..4 {
                for j in i + 1..4 {
                    let mut q = p;
                    q.swap(i, j);
                    prop_assert_eq!(exact(&q), -exact(&p));
                    if let (Some(s), Some(t)) = (s, orient3d(&q)) {
                        prop_assert_eq!(t, -s, "{} {:?} swapped {} {}", family, p, i, j);
                    }
                }
            }
        }
    }

    #[test]
    fn an_even_permutation_keeps_the_sign(families in families()) {
        for (family, p) in families {
            let s = orient3d(&p);
            for permutation in permutations().into_iter().filter(|&s| is_even(s)) {
                let q = permuted(&p, permutation);
                prop_assert_eq!(exact(&q), exact(&p));
                if let (Some(s), Some(t)) = (s, orient3d(&q)) {
                    prop_assert_eq!(t, s, "{} {:?} {:?}", family, p, permutation);
                }
            }
        }
    }

    #[test]
    fn translating_a_grid_case_keeps_the_sign(
        p in prop_oneof![grid4(), coplanar4()],
        shift in [-64i32..=64, -64i32..=64, -64i32..=64],
    ) {
        // A translation by small multiples of the unit of the grid, the
        // largest power of two that divides every coordinate: every
        // coordinate stays a multiple of it with fewer than 53 bits, so the
        // translation is exact.
        let unit = p.iter().flatten().filter_map(|&x| grid_unit(x)).reduce(f64::min);
        prop_assume!(unit.is_some());
        let unit = unit.unwrap();
        let q = p.map(|point| std::array::from_fn(|axis| point[axis] + f64::from(shift[axis]) * unit));
        for (point, moved) in p.iter().zip(&q) {
            for axis in 0..3 {
                let expected = &Dyadic::from_f64(point[axis])
                    + &(&Dyadic::from_f64(f64::from(shift[axis])) * &Dyadic::from_f64(unit));
                prop_assert!(Dyadic::from_f64(moved[axis]) == expected, "{:?} {:?}", p, shift);
            }
        }
        prop_assert_eq!(exact(&q), exact(&p));
        prop_assert_eq!(orient3d(&q), Some(exact(&p)), "{:?} moved by {:?}", p, shift);
    }

    #[test]
    fn negative_zero_and_repeated_points(
        p in prop_oneof![grid4(), random4(), signed_zeros4()],
        from in 0usize..4,
        to in 0usize..4,
    ) {
        // Negative zero is zero.
        let flipped = p.map(|point| point.map(|x| if x == 0.0 { -x } else { x }));
        prop_assert_eq!(orient3d(&flipped), orient3d(&p));
        for (name, stage) in STAGES {
            prop_assert_eq!(stage(refs(&flipped)), stage(refs(&p)), "{}", name);
        }
        // A repeated point makes the points coplanar, and every exact stage
        // sees it.
        if from != to {
            let mut q = p;
            q[to] = q[from];
            prop_assert_eq!(orient3d(&q), Some(Sign::Zero), "{:?}", q);
            prop_assert_eq!(stages::filter(refs(&q), 0.0), None);
        }
    }

    #[test]
    fn non_finite_inputs_are_none_and_do_not_panic(
        p in prop_oneof![random4(), grid4(), extreme4(&edges())],
    ) {
        for point in 0..4 {
            for axis in 0..3 {
                for bad in [f64::NAN, -f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    let mut q = p;
                    q[point][axis] = bad;
                    for (name, stage) in STAGES {
                        prop_assert_eq!(stage(refs(&q)), None, "{} on {:?}", name, q);
                    }
                    prop_assert_eq!(orient3d(&q), None);
                }
            }
        }
    }
}

/// The documented range of `orient3d` is the range of the untranslated
/// stage because it contains the range of the translated one.
#[test]
fn the_untranslated_range_contains_the_translated_range() {
    let (t, u) = (stages::TRANSLATED_RANGE, stages::UNTRANSLATED_RANGE);
    assert!(u.0 <= t.0 && t.1 <= u.1, "{t:?} {u:?}");
}
