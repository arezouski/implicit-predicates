//! The requirements of the expansion primitives that debug builds check:
//! the product domain, finite values and the output capacity.

use indirect_predicates_core::expansion;

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn product_constructor_checks_the_domain_in_debug_builds() {
    let mut out = [0.0; 2];
    let _ = expansion::from_two_product(
        indirect_predicates_core::eft::PRODUCT_OPERAND_MAX * 2.0,
        1.0,
        &mut out,
    );
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn scale_checks_its_components_in_debug_builds() {
    let mut out = [0.0; 4];
    let _ = expansion::scale(
        &[
            indirect_predicates_core::eft::PRODUCT_OPERAND_MIN / 2.0,
            1.0,
        ],
        3.0,
        &mut out,
    );
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn scale_checks_its_factor_in_debug_builds() {
    let mut out = [0.0; 4];
    let _ = expansion::scale(
        &[0.25, 1.0],
        indirect_predicates_core::eft::PRODUCT_OPERAND_MAX * 2.0,
        &mut out,
    );
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not finite")]
fn from_f64_checks_finiteness_in_debug_builds() {
    let mut out = [0.0; 1];
    let _ = expansion::from_f64(f64::NAN, &mut out);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not finite")]
fn grow_checks_finiteness_in_debug_builds() {
    let mut out = [0.0; 2];
    let _ = expansion::grow(&[1.0], f64::INFINITY, &mut out);
}

/// The result needs two components and the buffer holds one.
fn sum_into_a_buffer_too_small() {
    let mut out = [0.0; 1];
    let _ = expansion::sum(&[1e-30], &[1.0], &mut out);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "capacity")]
fn insufficient_capacity_fails_the_capacity_check() {
    sum_into_a_buffer_too_small();
}

#[cfg(not(debug_assertions))]
#[test]
#[should_panic(expected = "out of bounds")]
fn insufficient_capacity_panics_instead_of_writing_past_the_buffer() {
    sum_into_a_buffer_too_small();
}
