use super::Dyadic;

fn d(mantissa: i64, exponent: i64) -> Dyadic {
    Dyadic::new(mantissa, exponent)
}

fn p2(e: i64) -> Dyadic {
    Dyadic::pow2(e)
}

/// Asserts that rounding `x` up and down gives `up` and `down`.
fn rounds(x: &Dyadic, down: f64, up: f64) {
    assert_eq!(x.round_down().to_bits(), down.to_bits(), "down of {x:?}");
    assert_eq!(x.round_up().to_bits(), up.to_bits(), "up of {x:?}");
}

#[test]
fn values_are_normalized() {
    assert_eq!(d(4, 0), Dyadic::integer(4));
    assert_eq!(d(4, 0), p2(2));
    assert_eq!(d(12, -3), d(3, -1));
    assert_eq!(d(0, 7), Dyadic::integer(0));
}

#[test]
fn arithmetic_is_exact() {
    assert_eq!(&Dyadic::integer(3) + &p2(-2), d(13, -2));
    assert_eq!(&p2(-2) - &Dyadic::integer(3), d(-11, -2));
    assert_eq!(&d(3, -1) * &d(-5, 4), d(-15, 3));
    assert_eq!(&(&p2(100) + &p2(-100)) - &p2(100), p2(-100));
    assert_eq!(&d(7, 0) - &d(7, 0), Dyadic::integer(0));
}

#[test]
fn comparison_is_exact() {
    assert!(p2(-1000) > Dyadic::integer(0));
    assert!(&Dyadic::integer(1) + &p2(-200) > Dyadic::integer(1));
    assert!(d(-3, 5) < d(-3, 4));
    assert!(d(5, -1) == d(10, -2));
}

#[test]
fn ceil_log2_is_the_smallest_power_of_two_above() {
    for (x, s) in [
        (d(1, 0), 0),
        (d(1, -7), -7),
        (d(3, 0), 2),
        (d(5, -3), 0),
        (&p2(10) + &p2(-90), 11),
        (&p2(10) - &p2(-90), 10),
    ] {
        assert_eq!(x.ceil_log2(), s, "{x:?}");
        assert!(x <= p2(s) && x > p2(s - 1), "{x:?}");
    }
}

#[test]
fn fractions_are_in_lowest_terms() {
    assert_eq!(d(13, -2).fraction(), "13/4");
    assert_eq!(d(-3, 0).fraction(), "-3");
    assert_eq!(d(3, 2).fraction(), "12");
    assert_eq!(d(6, -3).fraction(), "3/4");
    assert_eq!(Dyadic::integer(0).fraction(), "0");
    assert_eq!(p2(-53).fraction(), "1/9007199254740992");
}

#[test]
fn representable_values_round_to_themselves() {
    for x in [
        0.0,
        0.75,
        1.0,
        -2.5,
        f64::MAX,
        -f64::MAX,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::from_bits(3),
        2f64.powi(53),
        2f64.powi(53) - 1.0,
    ] {
        rounds(&Dyadic::from_f64(x), x, x);
    }
}

#[test]
fn values_next_to_powers_of_two_round_to_the_neighbours() {
    for k in [-1000i64, -60, -1, 0, 1, 52, 53, 900] {
        let power = 2f64.powi(k as i32);
        let above = &p2(k) + &p2(k - 70);
        rounds(&above, power, power.next_up());
        let below = &p2(k) - &p2(k - 70);
        rounds(&below, power.next_down(), power);
        rounds(&(&Dyadic::integer(0) - &above), -power.next_up(), -power);
    }
}

#[test]
fn values_wider_than_53_bits_round_both_ways() {
    let two53 = 2f64.powi(53);
    rounds(&d((1 << 53) + 1, 0), two53, two53 + 2.0);
    rounds(&d(-(1 << 53) - 1, 0), -two53 - 2.0, -two53);
    // 2^60 + 2^7 + 1 needs 61 bits.
    let x = d((1 << 60) + (1 << 7) + 1, -10);
    let low = 2f64.powi(50) + 2f64.powi(-3);
    rounds(&x, low, low.next_up());
}

#[test]
fn subnormal_and_tiny_values_round_to_the_grid_of_2_pow_minus_1074() {
    let tiny = f64::from_bits(1);
    rounds(&d(3, -1075), tiny, 2.0 * tiny);
    rounds(&p2(-2000), 0.0, tiny);
    let negative = &Dyadic::integer(0) - &p2(-2000);
    assert_eq!(negative.round_up(), 0.0);
    assert_eq!(negative.round_down(), -tiny);
}

#[test]
fn values_beyond_the_range_round_to_max_or_infinity() {
    rounds(&p2(1024), f64::MAX, f64::INFINITY);
    rounds(
        &(&Dyadic::from_f64(f64::MAX) + &p2(900)),
        f64::MAX,
        f64::INFINITY,
    );
    let negative = &Dyadic::integer(0) - &p2(5000);
    rounds(&negative, f64::NEG_INFINITY, -f64::MAX);
}

#[test]
fn rounding_brackets_the_value_with_adjacent_floats() {
    // A deterministic sample of mantissas of up to 120 bits over the whole
    // exponent range, without overflow.
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..20_000 {
        let bits = next() % 120 + 1;
        let high = num_bigint::BigInt::from(next()) << 64u32;
        let mut mantissa = (high + num_bigint::BigInt::from(next())) >> (128 - bits);
        if next() % 2 == 0 {
            mantissa = -mantissa;
        }
        let exponent = (next() % 2100) as i64 - 1150 - bits as i64;
        let x = Dyadic::new(mantissa, exponent);
        let (down, up) = (x.round_down(), x.round_up());
        assert!(
            Dyadic::from_f64(down) <= x && x <= Dyadic::from_f64(up),
            "{x:?}"
        );
        if Dyadic::from_f64(down) == x {
            assert_eq!(down.to_bits(), up.to_bits(), "{x:?}");
        } else {
            assert_eq!(down.next_up(), up, "{x:?}");
        }
    }
}
