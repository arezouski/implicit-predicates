//! Valid expansions built directly from bit patterns, without the arithmetic
//! under test.
//!
//! The expansions of [`crate::expansion_with`] are sums computed by
//! `expansion::grow`, so they only have the shapes that `grow` produces. In
//! practice those lack *adjacent* components, whose bits touch without
//! overlapping (as in `[3.0, 4.0]` or `[1.0, 2.0, 4.0, 8.0]`), and have at
//! most about 9 components. The generators here place each component's bits
//! explicitly, so they reach every shape the expansion invariant admits.

use proptest::prelude::*;

use crate::checks::{highest_bit, integer_significand, lowest_bit};

/// The shape of the significand of one component.
#[derive(Clone, Copy, Debug)]
enum Pattern {
    /// A power of two.
    SingleBit,
    /// 53 significant bits (fewer only when the range runs out), with the
    /// bits between the lowest and the highest random.
    Full,
    /// A random number of bits, all set.
    AllOnes,
    /// A random number of bits, the lowest and the highest set and the
    /// others random.
    Random,
}

/// The description of one component: its pattern, its width in bits when
/// the pattern does not fix it, the random bits of its interior, the
/// distance from the highest bit of the previous component to its own
/// lowest bit, and its sign.
type ComponentSpec = (Pattern, u32, u64, i32, bool);

/// A description of one component, with gap 1 (adjacent components) in more
/// than half of the cases.
fn component_spec() -> impl Strategy<Value = ComponentSpec> {
    let pattern = prop_oneof![
        Just(Pattern::SingleBit),
        Just(Pattern::Full),
        Just(Pattern::AllOnes),
        Just(Pattern::Random),
    ];
    let gap = prop_oneof![3 => Just(1), 2 => 2..=80];
    (pattern, 1u32..=53, any::<u64>(), gap, any::<bool>())
}

/// A valid expansion of `0..=max_len` components whose set bits all lie in
/// `2^min_exp ..= 2^max_exp` (with `-1074 <= min_exp <= max_exp <= 1023`), so
/// every nonzero magnitude is in `2^min_exp .. 2^(max_exp + 1)`.
///
/// Components are laid out from the lowest up. Each is a power of two, a
/// full 53-bit significand, a run of set bits or a random pattern, with a
/// random sign; the next one starts one bit above it (adjacent) in more than
/// half of the cases and up to 80 bits above it otherwise. Half of the cases
/// ask for at least `max_len / 2` components, and most expansions start in
/// the lower half of the range, so long expansions are common when the range
/// is wide; an expansion stops early when the range runs out.
pub fn bit_pattern_expansion(
    max_len: usize,
    min_exp: i32,
    max_exp: i32,
) -> impl Strategy<Value = Vec<f64>> {
    assert!(
        -1074 <= min_exp && min_exp <= max_exp && max_exp <= 1023,
        "the bits must lie within those of an f64"
    );
    let middle = min_exp + (max_exp - min_exp) / 2;
    let start = prop_oneof![Just(min_exp), min_exp..=middle, min_exp..=max_exp];
    let count = prop_oneof![0..=max_len, max_len / 2..=max_len];
    let specs = prop::collection::vec(component_spec(), max_len);
    (start, count, specs)
        .prop_map(move |(start, count, specs)| lay_out(start, max_exp, &specs[..count]))
}

/// Pairs `(e, f)` of expansions from [`bit_pattern_expansion`]: in a third
/// of the cases independent, otherwise `f` is a *near copy* of `e` or of
/// `-e`, so that `e - f` or `e + f` cancels deeply or exactly.
///
/// A near copy is an exact copy, a copy without its lowest one to three
/// components, or a copy with one bit flipped strictly inside one component
/// (which keeps the component's lowest and highest bits, so the copy stays
/// valid).
pub fn bit_pattern_pair(
    max_len: usize,
    min_exp: i32,
    max_exp: i32,
) -> impl Strategy<Value = (Vec<f64>, Vec<f64>)> {
    let expansion = move || bit_pattern_expansion(max_len, min_exp, max_exp);
    let change = prop_oneof![
        Just(Change::None),
        (1usize..=3).prop_map(Change::DropLowest),
        (any::<usize>(), any::<u32>()).prop_map(|(i, bit)| Change::FlipInnerBit(i, bit)),
    ];
    prop_oneof![
        1 => (expansion(), expansion()),
        2 => (expansion(), change, any::<bool>())
            .prop_map(|(e, change, negated)| {
                let f = near_copy(&e, change, negated);
                (e, f)
            }),
    ]
}

/// How a near copy differs from the original.
#[derive(Clone, Copy, Debug)]
enum Change {
    None,
    /// Drop that many of the lowest components.
    DropLowest(usize),
    /// Flip one bit strictly between the lowest and the highest set bits of
    /// a component, both chosen modulo what is available.
    FlipInnerBit(usize, u32),
}

/// `e`, negated if `negated`, with `change` applied.
fn near_copy(e: &[f64], change: Change, negated: bool) -> Vec<f64> {
    let mut f: Vec<f64> = e.iter().map(|&x| if negated { -x } else { x }).collect();
    match change {
        Change::None => {}
        Change::DropLowest(count) => {
            f.drain(..count.min(f.len()));
        }
        Change::FlipInnerBit(i, bit) => {
            if let Some(x) = f.get_mut(i % e.len().max(1)) {
                *x = with_inner_bit_flipped(*x, bit);
            }
        }
    }
    f
}

/// `x` with one of the bits strictly between its lowest and highest set
/// bits flipped, chosen by `choice`, or `x` itself if there is none.
fn with_inner_bit_flipped(x: f64, choice: u32) -> f64 {
    let (low, high) = (lowest_bit(x), highest_bit(x));
    let inner = (high - low - 1).max(0).unsigned_abs();
    if inner == 0 {
        return x;
    }
    let position = low + 1 + i32::try_from(choice % inner).expect("below 53");
    let (_, unit) = integer_significand(x);
    let index = u32::try_from(position - unit).expect("an inner bit lies above the lowest bit");
    f64::from_bits(x.to_bits() ^ (1 << index))
}

/// Builds the components described by `specs`, the first with its lowest
/// bit at `2^start`, stopping when the next one would start above
/// `2^max_exp`; a component that would reach above it is narrowed.
fn lay_out(start: i32, max_exp: i32, specs: &[ComponentSpec]) -> Vec<f64> {
    let mut e = Vec::new();
    let mut low = start;
    for &(pattern, random_width, bits, gap, negative) in specs {
        let room = max_exp - low + 1;
        if room <= 0 {
            break;
        }
        let wanted = match pattern {
            Pattern::SingleBit => 1,
            Pattern::Full => 53,
            Pattern::AllOnes | Pattern::Random => random_width,
        };
        let width = wanted.min(room.unsigned_abs());
        let all_ones = u64::MAX >> (64 - width);
        let significand = match pattern {
            Pattern::SingleBit => 1,
            Pattern::AllOnes => all_ones,
            Pattern::Full | Pattern::Random => (bits & all_ones) | 1 | (1 << (width - 1)),
        };
        e.push(from_parts(significand, low, negative));
        low += i32::try_from(width).expect("at most 53") - 1 + gap;
    }
    e
}

/// The `f64` equal to `significand * 2^low`, negated if `negative`, where
/// `significand` is odd and below `2^53` and the value is representable
/// exactly. Built from the bit pattern, without floating-point arithmetic.
fn from_parts(significand: u64, low: i32, negative: bool) -> f64 {
    assert!(significand % 2 == 1 && significand < 1 << 53);
    let width = i32::try_from(64 - significand.leading_zeros()).expect("at most 64");
    let high = low + width - 1;
    assert!(
        low >= -1074 && high <= 1023,
        "bits {low}..={high} out of range"
    );
    let magnitude = if high >= -1022 {
        let biased = u64::try_from(high + 1023).expect("normal, so positive");
        let shift = u32::try_from(53 - width).expect("at most 53 bits");
        (biased << 52) | ((significand << shift) & ((1 << 52) - 1))
    } else {
        let shift = u32::try_from(low + 1074).expect("at least -1074");
        significand << shift
    };
    f64::from_bits((u64::from(negative) << 63) | magnitude)
}
