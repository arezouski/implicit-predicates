//! Self-tests of the shared test infrastructure: the expansion validity check
//! must reject each kind of invalid input, and the generators must produce
//! the kinds of inputs they promise.

use implicit_predicates_core::{Scratch, eft};
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{
    ArenaOp, Points4, arena_op, assert_valid_expansion, bit_pattern_expansion, bit_pattern_pair,
    coplanar4, correlated_pair, expansion, has_adjacent_components, near_coplanar4, near_equal,
    product_domain_bits, product_operand, repeated4, sample, value,
};

#[test]
#[should_panic(expected = "overlap or are out of order")]
fn rejects_equal_components() {
    assert_valid_expansion(&[1.0, 1.0]);
}

#[test]
#[should_panic(expected = "overlap or are out of order")]
fn rejects_decreasing_magnitudes() {
    assert_valid_expansion(&[2.0, 1.0]);
}

#[test]
#[should_panic(expected = "is zero or not finite")]
fn rejects_zero_components() {
    assert_valid_expansion(&[0.0, 1.0]);
}

#[test]
#[should_panic(expected = "is zero or not finite")]
fn rejects_non_finite_components() {
    assert_valid_expansion(&[1.0, f64::INFINITY]);
}

#[test]
#[should_panic(expected = "overlap or are out of order")]
fn rejects_overlap_when_the_smaller_has_several_bits() {
    assert_valid_expansion(&[3.0, 5.0]);
}

#[test]
#[should_panic(expected = "overlap or are out of order")]
fn rejects_overlap_with_fractional_parts() {
    assert_valid_expansion(&[1.5, 2.5]);
}

#[test]
fn accepts_valid_expansions() {
    assert_valid_expansion(&[]);
    assert_valid_expansion(&[1.0, 2.0]);
    // Adjacent components (one bit position apart) do not overlap.
    assert_valid_expansion(&[3.0, 4.0]);
    assert_valid_expansion(&[1.5, 2.0]);
    assert_valid_expansion(&[5e-324, 1.0]);
    assert_valid_expansion(&[-1.0, 2.0, -8.0]);
}

#[test]
fn generated_expansions_often_have_several_components() {
    // Components whose bits all fit in 53 consecutive positions merge into
    // one, so leaves within `2^-200 ..= 2^200` give at most about
    // 400 / 53 + 1 = 8 components.
    let samples = sample(expansion(16), 1000);
    let several = samples.iter().filter(|e| e.len() >= 3).count();
    let many = samples.iter().filter(|e| e.len() >= 6).count();
    assert!(
        several >= 700,
        "only {several} of 1000 have 3 or more components"
    );
    assert!(many >= 150, "only {many} of 1000 have 6 or more components");
}

#[test]
fn correlated_pairs_usually_round() {
    let samples = sample(correlated_pair(), 1000);
    let rounded = samples
        .iter()
        .filter(|&&(a, b)| eft::two_sum(a, b).1 != 0.0)
        .count();
    assert!(rounded >= 850, "only {rounded} of 1000 sums round");
}

#[test]
fn near_equal_pairs_cancel_deeply() {
    let samples = sample(near_equal(8, 100), 1000);
    let two_54 = Dyadic::from_f64(2f64.powi(54));
    let abs = |d: Dyadic| if d.sign() < 0 { -d } else { d };
    let mut cancelling = 0;
    for (e, f) in &samples {
        assert_valid_expansion(e);
        assert_valid_expansion(f);
        let difference = abs(&value(e) - &value(f));
        assert!(
            &difference * &two_54 < abs(value(e)) || e.is_empty(),
            "{e:?} and {f:?} are not near-equal"
        );
        if !e.is_empty() && !difference.is_zero() {
            cancelling += 1;
        }
    }
    assert!(
        cancelling >= 850,
        "only {cancelling} of 1000 pairs differ and cancel"
    );
}

#[test]
fn product_operands_lie_in_the_product_domain() {
    let samples = sample(product_operand(), 5000);
    assert!(samples.iter().all(|&x| eft::in_product_domain(x)));
    let at_bounds = samples
        .iter()
        .filter(|&&x| x.abs() == eft::PRODUCT_OPERAND_MIN || x.abs() == eft::PRODUCT_OPERAND_MAX)
        .count();
    assert!(at_bounds > 0, "the bounds themselves are never drawn");
}

#[test]
fn adjacency_is_detected() {
    assert!(has_adjacent_components(&[3.0, 4.0]));
    assert!(has_adjacent_components(&[0.5, 1.0, 8.0]));
    assert!(!has_adjacent_components(&[1.0, 4.0]));
    assert!(!has_adjacent_components(&[1.0]));
}

/// Whether the bits of `x`'s significand, from its lowest to its highest set
/// bit, are all set.
fn is_run_of_ones(x: f64) -> bool {
    let fraction = x.to_bits() & ((1 << 52) - 1);
    let significand = if x.abs() >= f64::MIN_POSITIVE {
        fraction | (1 << 52)
    } else {
        fraction
    };
    let shifted = significand >> significand.trailing_zeros();
    shifted & (shifted + 1) == 0
}

#[test]
fn bit_pattern_expansions_are_valid_long_and_mostly_adjacent() {
    let samples = sample(bit_pattern_expansion(40, -1074, 1023), 1000);
    for e in &samples {
        assert_valid_expansion(e);
    }
    let adjacent = samples
        .iter()
        .filter(|e| has_adjacent_components(e))
        .count();
    let long = samples.iter().filter(|e| e.len() >= 16).count();
    let components: Vec<f64> = samples.iter().flatten().copied().collect();
    let powers = components
        .iter()
        .filter(|x| x.abs().to_bits() & ((1 << 52) - 1) == 0)
        .count();
    let full = components
        .iter()
        .filter(|x| x.to_bits() & 1 == 1 && x.abs() >= f64::MIN_POSITIVE)
        .count();
    let ones = components
        .iter()
        .filter(|&&x| is_run_of_ones(x) && x.abs().to_bits() & ((1 << 52) - 1) != 0)
        .count();
    let subnormal = components
        .iter()
        .filter(|x| x.abs() < f64::MIN_POSITIVE)
        .count();
    println!(
        "adjacent: {adjacent}, long: {long}, of {} components: {powers} powers, {full} full, \
         {ones} runs of ones, {subnormal} subnormal",
        components.len()
    );
    assert!(
        adjacent >= 600,
        "only {adjacent} of 1000 have adjacent components"
    );
    assert!(
        long >= 250,
        "only {long} of 1000 have 16 or more components"
    );
    for (kind, count) in [
        ("powers", powers),
        ("full", full),
        ("runs of ones", ones),
        ("subnormal", subnormal),
    ] {
        assert!(count >= 100, "only {count} components are {kind}");
    }
}

#[test]
fn bit_pattern_expansions_stay_in_the_requested_bits() {
    let (low, high) = product_domain_bits();
    for e in sample(bit_pattern_expansion(40, low, high), 1000) {
        assert_valid_expansion(&e);
        assert!(e.iter().all(|&x| eft::in_product_domain(x)), "{e:?}");
    }
    let (min, max) = (2f64.powi(-30), 2f64.powi(31));
    for e in sample(bit_pattern_expansion(12, -30, 30), 1000) {
        assert!(e.iter().all(|x| (min..max).contains(&x.abs())), "{e:?}");
    }
}

#[test]
fn bit_pattern_pairs_often_cancel_deeply() {
    let samples = sample(bit_pattern_pair(40, -1074, 1019), 1000);
    let two_50 = Dyadic::from_f64(2f64.powi(50));
    let abs = |d: Dyadic| if d.sign() < 0 { -d } else { d };
    let (mut deep, mut exact) = (0, 0);
    for (e, f) in &samples {
        assert_valid_expansion(e);
        assert_valid_expansion(f);
        let (ve, vf) = (value(e), value(f));
        let smaller = abs(&ve + &vf).min(abs(&ve - &vf));
        deep += usize::from(!e.is_empty() && &smaller * &two_50 < abs(ve));
        exact += usize::from(!e.is_empty() && smaller.is_zero());
    }
    println!("deep: {deep}, exact: {exact}");
    assert!(deep >= 400, "only {deep} of 1000 pairs cancel deeply");
    assert!(exact >= 100, "only {exact} of 1000 pairs cancel exactly");
}

#[test]
fn arena_sequences_run_every_kind_of_operation() {
    let mut buf = vec![0.0; 1 << 16];
    let (mut scales, mut products, mut multi_component_products) = (0, 0, 0);
    for ops in sample(proptest::collection::vec(arena_op(), 10), 300) {
        let mut scratch = Scratch::borrowed(&mut buf);
        let mut spans = Vec::new();
        for op in &ops {
            let Some(applied) = op.apply(&mut scratch, &spans) else {
                continue;
            };
            let span = applied.result.expect("the arena is large enough");
            match op {
                ArenaOp::Scale(..) => scales += 1,
                ArenaOp::Product(..) => {
                    products += 1;
                    let lengths = applied.operands.iter().map(|&i| spans[i].len());
                    multi_component_products += usize::from(lengths.min() >= Some(2));
                }
                _ => {}
            }
            spans.push(span);
        }
    }
    println!("scales: {scales}, products: {products} ({multi_component_products} multi-component)");
    assert!(scales >= 200, "only {scales} scales ran");
    assert!(products >= 150, "only {products} products ran");
    assert!(
        multi_component_products >= 50,
        "only {multi_component_products} multi-component products ran"
    );
}

/// The exact determinant of `[a - d; b - d; c - d]`.
fn orientation(p: &Points4) -> Dyadic {
    let row = |i: usize| -> [Dyadic; 3] {
        std::array::from_fn(|axis| &Dyadic::from_f64(p[i][axis]) - &Dyadic::from_f64(p[3][axis]))
    };
    let (a, b, c) = (row(0), row(1), row(2));
    let minor = |x: usize, y: usize| &(&b[x] * &c[y]) - &(&b[y] * &c[x]);
    &(&(&a[0] * &minor(1, 2)) - &(&a[1] * &minor(0, 2))) + &(&a[2] * &minor(0, 1))
}

#[test]
fn coplanar_points_are_exactly_coplanar() {
    for p in sample(coplanar4(), 2000) {
        assert!(orientation(&p).is_zero(), "{p:?}");
    }
}

#[test]
fn nearly_coplanar_points_are_rarely_coplanar() {
    let cases = sample(near_coplanar4(), 2000);
    let coplanar = cases.iter().filter(|p| orientation(p).is_zero()).count();
    // Only when every moved coordinate has a zero cofactor, as when it
    // belongs to a repeated point.
    assert!(
        coplanar * 10 < cases.len(),
        "{coplanar} of {} coplanar",
        cases.len()
    );
}

#[test]
fn repeated_points_repeat_a_point() {
    for p in sample(repeated4(), 2000) {
        let repeats = (0..4).any(|i| (i + 1..4).any(|j| p[i] == p[j]));
        assert!(repeats, "{p:?}");
    }
}
