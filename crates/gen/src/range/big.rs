//! The size of the arena of the stage on big integers, for explicit points.
//!
//! The stage evaluates the main form node by node with the big-integer
//! operations of the arena: a leaf is `big_sub` of two coordinates, a sum,
//! difference or product is `big_add`, `big_sub` or `big_mul`, and a negation
//! is `neg`, which writes nothing. Every result stays in the arena until the
//! stage returns. Whoever changes this scheme changes the model here.
//!
//! # Why the size is sufficient
//!
//! A big integer is a string of 64-bit words, one per `f64` slot of the
//! arena, scaled by a power of `2^64`; its words run from the one that holds
//! its lowest set bit to the one that holds its highest. A value whose set
//! bits all lie between the positions `l` and `h` takes at most
//! `floor(h / 64) - floor(l / 64) + 1 <= ceil((h - l) / 64) + 1` words.
//!
//! An explicit coordinate is a finite `f64`: a multiple of `2^-1074` of
//! magnitude below `2^1024`, so it takes at most 2 words, and the points
//! take `6 points` in all. Read the main form as a polynomial in the
//! coordinates, and let `c` bound the sum of the magnitudes of its
//! coefficients at a node: 2 for a leaf, the operand's for a negation,
//! `c_x + c_y` for a sum or difference and `c_x c_y` for a product. The
//! value of a node of degree `g` is then a multiple of `2^(-1074 g)` of
//! magnitude below `c 2^(1024 g)`, its set bits lie between `-1074 g` and
//! `1024 g + log2 c`, and it takes at most
//! `W = ceil((2098 g + b) / 64) + 1` words, with `b >= log2 c`: 1 for a
//! leaf, the operand's for a negation, `max(b_x, b_y) + 1` for a sum or
//! difference and `b_x + b_y` for a product.
//!
//! While it runs, an operation reserves more than its result: `big_add` and
//! `big_sub` reserve the words from the lowest word of either operand to the
//! highest, plus one for a carry. Both operands of a sum have the degree of
//! the sum and bits below `b`, so that is at most `W + 1` words, and the
//! same holds for a leaf, whose operands are coordinates. `big_mul` reserves
//! the lengths of its operands together, at most `W_x + W_y`, which can be
//! one more than `W + 1`. The reservation of each node is also a bound on
//! its result, and the arena holds at any moment the coordinates, the
//! results of the nodes evaluated before and the reservation of the node
//! being evaluated; so the coordinates and the sum of the reservations of
//! all the nodes, each counted once as the stage evaluates it once, bound
//! the arena that the stage takes.

use crate::differences::{DiffGraph, DiffNode};
use crate::error::Error;
use crate::parse::Position;

/// The words of an explicit coordinate as a big integer.
const COORDINATE_WORDS: u64 = 2;

/// The bits of the grid and of the magnitude of a value of degree one: a
/// multiple of `2^-1074` below `2^1024`.
const BITS_PER_DEGREE: u64 = 1074 + 1024;

/// The `f64` slots of a `Scratch` arena, one 64-bit word each, that suffice
/// for the stage on big integers of `diff` with `points` explicit points, the
/// coordinates of the points as big integers included. See the
/// documentation of this module for the model and why it is sufficient; it
/// is not tight.
///
/// # Errors
///
/// Returns an error, at `position` in `file`, when the size exceeds
/// `u32::MAX`, the most words that one operation of the arena can reserve.
pub(crate) fn big_explicit_arena(
    diff: &DiffGraph,
    points: usize,
    file: &str,
    position: Position,
) -> Result<usize, Error> {
    let too_large = || {
        Error::at(
            file,
            position,
            format!(
                "the stage on big integers needs an arena of more than {} words for explicit \
                 points",
                u32::MAX
            ),
        )
    };
    let mut bits: Vec<u64> = Vec::with_capacity(diff.nodes.len());
    let mut words: Vec<u64> = Vec::with_capacity(diff.nodes.len());
    let mut total = u64::try_from(points)
        .ok()
        .and_then(|p| p.checked_mul(3 * COORDINATE_WORDS))
        .ok_or_else(too_large)?;
    for (node, &degree) in diff.nodes.iter().zip(&diff.degrees) {
        let b = match *node {
            DiffNode::Leaf { .. } => Some(1),
            DiffNode::Neg(x) => Some(bits[x]),
            DiffNode::Add(x, y) | DiffNode::Sub(x, y) => bits[x].max(bits[y]).checked_add(1),
            DiffNode::Mul(x, y) => bits[x].checked_add(bits[y]),
        }
        .ok_or_else(too_large)?;
        let w = BITS_PER_DEGREE
            .checked_mul(u64::from(degree))
            .and_then(|n| n.checked_add(b))
            .map(|n| n.div_ceil(64))
            .and_then(|n| n.checked_add(1))
            .ok_or_else(too_large)?;
        let reserved = match *node {
            DiffNode::Neg(_) => Some(0),
            DiffNode::Mul(x, y) => words[x].checked_add(words[y]),
            DiffNode::Leaf { .. } | DiffNode::Add(..) | DiffNode::Sub(..) => w.checked_add(1),
        }
        .ok_or_else(too_large)?;
        total = total.checked_add(reserved).ok_or_else(too_large)?;
        if total > u64::from(u32::MAX) {
            return Err(too_large());
        }
        bits.push(b);
        words.push(w);
    }
    usize::try_from(total).map_err(|_| too_large())
}

#[cfg(test)]
mod tests {
    use implicit_predicates_core::{Big, Scratch, ScratchExhausted, Sign};

    use super::big_explicit_arena;
    use crate::differences::{DiffGraph, DiffNode, difference_graph};
    use crate::parse::{Position, parse};

    const ORIENT3D: &str = include_str!("../../tests/fixtures/orient3d.pred");
    const INSPHERE: &str = include_str!("../../tests/fixtures/insphere.pred");
    const AT: Position = Position { line: 3, column: 4 };

    fn graph(source: &str) -> (DiffGraph, usize) {
        let p = parse(source, "t.pred").unwrap().remove(0);
        let points = p.params.len();
        (
            difference_graph(&p.main, "t.pred", p.position).unwrap(),
            points,
        )
    }

    fn arena(source: &str) -> usize {
        let (g, points) = graph(source);
        big_explicit_arena(&g, points, "t.pred", AT).unwrap()
    }

    #[test]
    fn a_leaf_reserves_one_word_more_than_a_difference_of_opposite_extremes() {
        // Two points of 6 words, and 34 + 1 for the leaf: a difference of
        // coordinates near `2^1024` and `2^-1074` spans words -17 to 16.
        let source = "fn p(a: Point3, b: Point3) -> Sign { a.x - b.x }";
        assert_eq!(arena(source), 12 + 35);
    }

    #[test]
    fn a_negation_reserves_nothing() {
        let source = "fn p(a: Point3, b: Point3) -> Sign { -(a.x - b.x) }";
        assert_eq!(arena(source), 12 + 35);
    }

    #[test]
    fn a_product_reserves_the_words_of_its_operands() {
        // Each leaf 34 words (35 reserved), the product 34 + 34.
        let source = "fn p(a: Point3, b: Point3) -> Sign { (a.x - b.x) * (a.y - b.y) }";
        assert_eq!(arena(source), 12 + 35 + 35 + 68);
    }

    #[test]
    fn a_sum_reserves_one_word_more_than_its_bound() {
        // Products of degree 2 and 2 bits of coefficients take
        // `ceil(4198 / 64) + 1 = 67` words; their sum has 3 bits and takes
        // `ceil(4199 / 64) + 1 = 67`, plus one reserved.
        let source = "fn p(a: Point3, b: Point3) -> Sign {
            let x = a.x - b.x;
            let y = a.y - b.y;
            x * y + y * x
        }";
        assert_eq!(arena(source), 12 + 35 + 35 + 68 + 68 + 68);
    }

    #[test]
    fn an_arena_beyond_u32_max_words_is_an_error_at_the_position() {
        let leaf = DiffNode::Leaf {
            axis: 0,
            minuend: 0,
            subtrahend: 1,
        };
        // Degrees no definition reaches, written directly: a sum of degree
        // `3 * 10^9` takes about `10^11` words.
        let g = DiffGraph {
            nodes: vec![leaf, DiffNode::Add(0, 0)],
            root: 1,
            degrees: vec![1, 3_000_000_000],
        };
        // The highest degree a graph can hold, on a leaf.
        let leaf_only = DiffGraph {
            nodes: vec![leaf],
            root: 0,
            degrees: vec![u32::MAX],
        };
        for (g, points) in [(&g, 2), (&leaf_only, 2), (&graph(ORIENT3D).0, usize::MAX)] {
            let e = big_explicit_arena(g, points, "t.pred", AT).unwrap_err();
            assert_eq!((e.file.as_str(), e.line, e.column), ("t.pred", 3, 4));
            assert!(e.message.contains("4294967295 words"), "{e}");
        }
    }

    /// Evaluates `g` as the stage does, on the coordinates `c`, in an arena
    /// of exactly `size` slots, and returns its sign and high-water mark.
    fn evaluate(
        g: &DiffGraph,
        c: &[[f64; 3]],
        size: usize,
    ) -> Result<(Sign, usize), ScratchExhausted> {
        let mut buf = vec![0.0; size];
        let mut scratch = Scratch::borrowed(&mut buf);
        let mut coords = Vec::new();
        for point in c {
            let mut big = [Big::ZERO; 3];
            for (b, &x) in big.iter_mut().zip(point) {
                *b = scratch.big_from_f64(x)?;
            }
            coords.push(big);
        }
        let mut values: Vec<Big> = Vec::new();
        for node in &g.nodes {
            let v = match *node {
                DiffNode::Leaf {
                    axis,
                    minuend,
                    subtrahend,
                } => {
                    let axis = usize::from(axis);
                    scratch.big_sub(coords[minuend][axis], coords[subtrahend][axis])?
                }
                DiffNode::Neg(x) => values[x].neg(),
                DiffNode::Add(x, y) => scratch.big_add(values[x], values[y])?,
                DiffNode::Sub(x, y) => scratch.big_sub(values[x], values[y])?,
                DiffNode::Mul(x, y) => scratch.big_mul(values[x], values[y])?,
            };
            values.push(v);
        }
        Ok((values[g.root].sign(), scratch.high_water()))
    }

    #[test]
    fn the_arena_suffices_for_coordinates_at_opposite_extremes() {
        let extremes = [
            f64::MAX,
            -f64::MAX,
            f64::from_bits(1),
            -f64::from_bits(1),
            f64::MIN_POSITIVE,
            -(1.0 + f64::EPSILON),
            0.0,
            3.0e-300,
        ];
        for source in [ORIENT3D, INSPHERE] {
            let (g, points) = graph(source);
            let size = big_explicit_arena(&g, points, "t.pred", AT).unwrap();
            let mut highest = 0;
            for shift in 0..64 {
                let c: Vec<[f64; 3]> = (0..points)
                    .map(|i| {
                        [0, 1, 2].map(|k| extremes[(i * 3 + k + shift * (i + 1)) % extremes.len()])
                    })
                    .collect();
                let (_, high) = evaluate(&g, &c, size).expect("the arena suffices");
                highest = highest.max(high);
            }
            assert!(highest <= size, "{highest} > {size}");
        }
    }
}
