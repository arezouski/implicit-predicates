//! The product of expansions on the scratch arena: exact, with the result
//! left at the previous top and the working space accounted for in the
//! high-water mark.

use indirect_predicates_core::{Mark, Scratch, ScratchExhausted, Span};
use indirect_predicates_test_support::{
    assert_valid_expansion, bit_pattern_expansion, config, expansion_within, product_domain_bits,
    product_working_space, value,
};
use proptest::prelude::*;

/// Expansions of up to 16 components from bit patterns, every component in
/// the product domain.
fn bit_pattern_operand() -> impl Strategy<Value = Vec<f64>> {
    let (low, high) = product_domain_bits();
    bit_pattern_expansion(16, low, high)
}

/// Checks that the product of `e` and `f` on an arena is exact and valid,
/// that it ends just above the operands and that its working space is the
/// documented one, and that the operands are untouched.
fn assert_exact_product(e: &[f64], f: &[f64]) {
    let mut buf = vec![f64::NAN; e.len() + f.len() + product_working_space(e.len(), f.len())];
    let mut s = Scratch::borrowed(&mut buf);
    let (se, sf) = (s.push(e).unwrap(), s.push(f).unwrap());
    let (used_before, mark) = (s.used(), s.mark());
    let p = s.product(se, sf).unwrap();
    assert_valid_expansion(s.get(p));
    assert_eq!(value(s.get(p)), &value(e) * &value(f));
    assert_eq!(s.used(), used_before + p.len());
    assert_eq!(
        s.high_water(),
        used_before + product_working_space(e.len(), f.len())
    );
    assert_starts_at(&mut s, mark, p);
    assert_eq!(s.get(se), e);
    assert_eq!(s.get(sf), f);
}

/// Checks that `p`, the last result, starts at `mark`, the top before the
/// operation that produced it: pushing a copy of its components at `mark`
/// gives a span equal to `p`, and leaves the arena as it was.
fn assert_starts_at(s: &mut Scratch<'_>, mark: Mark, p: Span) {
    let components = s.get(p).to_vec();
    s.reset(mark);
    assert_eq!(
        s.push(&components),
        Ok(p),
        "the result is not at the previous top"
    );
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn product_is_exact(e in expansion_within(8, 100), f in expansion_within(8, 100)) {
        assert_exact_product(&e, &f);
    }

    #[test]
    fn product_is_exact_on_bit_patterns(e in bit_pattern_operand(), f in bit_pattern_operand()) {
        assert_exact_product(&e, &f);
    }

    #[test]
    fn product_is_commutative_in_value(
        e in expansion_within(8, 100),
        f in expansion_within(8, 100),
    ) {
        let mut buf = vec![f64::NAN; 1024];
        let mut s = Scratch::borrowed(&mut buf);
        let (se, sf) = (s.push(&e).unwrap(), s.push(&f).unwrap());
        let ef = s.product(se, sf).unwrap();
        let fe = s.product(sf, se).unwrap();
        prop_assert_eq!(value(s.get(ef)), value(s.get(fe)));
        let difference = s.diff(ef, fe).unwrap();
        prop_assert!(difference.is_empty());
    }
}

#[test]
fn product_with_empty_operand_is_empty() {
    let mut buf = [0.0; 64];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s.push(&[1e-20, 1.0]).unwrap();
    let z = s.push(&[]).unwrap();
    assert!(s.product(a, z).unwrap().is_empty());
    assert!(s.product(z, a).unwrap().is_empty());
    assert!(s.product(z, z).unwrap().is_empty());
    // An empty operand needs no working space.
    assert_eq!(s.used(), 2);
    assert_eq!(s.high_water(), 2);
}

#[test]
fn product_exhaustion_leaves_no_partial_result() {
    // two 2-component operands need 4*2*2 + 2*2 = 20 above the 4 already used
    let mut buf = [0.0; 23];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s.push(&[1e-20, 1.0]).unwrap();
    let b = s.push(&[1e-25, 3.0]).unwrap();
    assert_eq!(s.product(a, b), Err(ScratchExhausted));
    assert_eq!(s.used(), 4);
    assert_eq!(s.high_water(), 4);
    assert_eq!(s.get(a), &[1e-20, 1.0]);
    assert_eq!(s.get(b), &[1e-25, 3.0]);
    let mut buf = [0.0; 24];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s.push(&[1e-20, 1.0]).unwrap();
    let b = s.push(&[1e-25, 3.0]).unwrap();
    assert!(s.product(a, b).is_ok());
    assert_eq!(s.high_water(), 24);
}

#[test]
fn product_result_sits_at_the_previous_top() {
    let tiny = 2f64.powi(-60);
    let mut buf = [f64::NAN; 64];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s.push(&[tiny, 1.0]).unwrap();
    let b = s.push(&[-tiny, 1.0]).unwrap();
    let c = s.push(&[3.0]).unwrap();
    // A shorter operand of two components accumulates in two steps and one
    // of one component in a single step, which leaves the result in the
    // other working zone; either way it must end up at the previous top.
    let cases = [
        (a, b, vec![-tiny * tiny, 1.0]),
        (a, c, vec![3.0 * tiny, 3.0]),
        (c, a, vec![3.0 * tiny, 3.0]),
    ];
    for (x, y, expected) in cases {
        let mark = s.mark();
        let p = s.product(x, y).unwrap();
        assert_eq!(value(s.get(p)), value(&expected));
        assert_eq!(s.used(), 5 + p.len());
        assert_starts_at(&mut s, mark, p);
        s.reset(mark);
    }
}
