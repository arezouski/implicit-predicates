//! The no-op call that measures the cost of the C interface itself.

use implicit_predicates_ffi_ref::{current, legacy};

/// The no-op answers 0 on any four points and checks its indices like a
/// predicate call.
macro_rules! assert_overhead_call {
    ($module:ident) => {{
        let mut points = $module::Points::new();
        let a = points.explicit([0.0, 0.0, 0.0]);
        let b = points.explicit([1.0, 0.0, 0.0]);
        let c = points.explicit([0.0, 1.0, 0.0]);
        let d = points.lnc(a, b, 0.25);
        assert_eq!(points.overhead(a, b, c, d), 0);
        assert_eq!(points.overhead(d, d, d, d), 0);
        let other = $module::Points::new();
        let missing =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| other.overhead(a, b, c, d)));
        assert!(
            missing.is_err(),
            "{}: a missing point must panic",
            stringify!($module)
        );
    }};
}

#[test]
fn the_no_op_call_answers_zero_and_checks_its_points() {
    assert_overhead_call!(legacy);
    assert_overhead_call!(current);
}
