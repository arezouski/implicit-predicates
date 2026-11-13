//! The functions of the checks module that check a stage on the edge
//! cases of the filter: `check_bound_edge_cases` and, for a per-axis
//! filter, `check_uncertainty_edge_cases`.
//!
//! They take the stage as a closure of the coordinates of the points (and
//! of their errors), so the tests run them on any stage of the chain and on
//! points of any type built from those coordinates, for the arity of the
//! predicate.

use std::fmt::Write;

use super::comment;

/// The most points for which `check_uncertainty_edge_cases` checks every
/// corner of the boxes of the errors: `2^12` corners per case, each an exact
/// evaluation. Above it, the number of corners doubles with each coordinate
/// (and from 22 points it does not fit in a `u64`), so the check takes a
/// fixed sample of `2 * (1 + 3 * points)` corners instead, and says so.
pub(crate) const MAX_EXHAUSTIVE_POINTS: usize = 4;

/// Writes the check functions for a predicate of `points` points; the one
/// with an uncertainty only for a per-axis filter, whose degrees in the axes
/// are `degrees`. `affine` says whether the predicate is affine in each
/// coordinate, which is when its sign at the corners of a box is its sign in
/// the whole box.
pub(crate) fn write_check_fns(
    out: &mut String,
    points: usize,
    degrees: Option<[u32; 3]>,
    affine: bool,
) {
    let ty = format!("[[f64; 3]; {points}]");
    out.push('\n');
    comment(
        out,
        "///",
        0,
        "Runs `decide` on every case of [`bound_edge_cases`] and checks that, whenever \
         it decides, its sign is the exact sign. Returns how many cases it decided and \
         how many it declined.\n\n\
         # Panics\n\n\
         Panics if `decide` gives a sign other than the exact sign.",
    );
    writeln!(
        out,
        "pub fn check_bound_edge_cases(mut decide: impl FnMut(&{ty}) -> Option<Sign>) \
         -> [usize; 2] {{\n\
         let mut counts = [0; 2];\n\
         for p in bound_edge_cases() {{\n\
         match decide(&p) {{\n\
         Some(sign) => {{\n\
         assert_eq!(sign, exact(&p), \"{{p:?}}\");\n\
         counts[0] += 1;\n\
         }}\n\
         None => counts[1] += 1,\n\
         }}\n\
         }}\n\
         counts\n\
         }}"
    )
    .unwrap();
    if let Some(degrees) = degrees {
        write_check_uncertainty(out, points, degrees, affine);
    }
}

/// Writes `check_uncertainty_edge_cases` for a per-axis filter of `points`
/// points whose degrees in the axes are `degrees`, for a predicate that is
/// affine in each coordinate or not, as `affine` says.
fn write_check_uncertainty(out: &mut String, points: usize, degrees: [u32; 3], affine: bool) {
    let ty = format!("[[f64; 3]; {points}]");
    let coords = 3 * points;
    let positive: Vec<bool> = degrees.iter().map(|&d| d > 0).collect();
    let every = if positive.iter().all(|&p| p) {
        "every axis".to_string()
    } else {
        let names: Vec<&str> = (["x", "y", "z"].iter().zip(&positive))
            .filter_map(|(name, &p)| p.then_some(*name))
            .collect();
        format!("every axis of positive degree, {}", names.join(" and "))
    };
    let sampled = points > MAX_EXHAUSTIVE_POINTS;
    let held = if sampled {
        "at a corner it checks"
    } else if affine {
        "in the boxes"
    } else {
        "at a corner"
    };
    let corners = if sampled {
        format!(
            "at {} of the `2^{coords}` corners of the boxes of the errors around the \
                 approximations: the corner with every coordinate at its lowest, the one \
                 with every coordinate at its highest, and each corner one coordinate away \
                 from either. With more than {MAX_EXHAUSTIVE_POINTS} points the corners are \
                 too many to check one by one, so this does not show that the sign holds in \
                 the whole boxes",
            2 * (1 + coords)
        )
    } else if affine {
        format!(
            "at each of the `2^{coords}` corners of the boxes of the errors around the \
             approximations; the predicate is affine in each coordinate, so it is then the \
             exact sign at every point within them"
        )
    } else {
        format!(
            "at each of the `2^{coords}` corners of the boxes of the errors around the \
             approximations. The predicate is not affine in each coordinate, so this does \
             not show that the sign holds in the whole boxes"
        )
    };
    out.push('\n');
    comment(
        out,
        "///",
        0,
        &format!(
            "Runs `decide` on every case of [`uncertainty_edge_cases`], given the \
             approximations `p` and an error of `e[k] / 2` for every point on each axis \
             `k`, so that the sum of the two largest errors on each axis is `e[k]`, and \
             checks that, whenever it decides, its sign is not zero and is the exact sign \
             {corners}. Returns how many cases it decided and how many it declined, for \
             `e` not zero on x only, on y only, on z only and on {every}.\n\n\
             # Panics\n\n\
             Panics if `decide` gives a sign that does not hold {held}."
        ),
    );
    // The arm of the cases with an uncertainty on every axis of positive
    // degree, unless there is only one such axis, whose arm it would be.
    let all = if positive.iter().filter(|&&p| p).count() > 1 {
        format!("{positive:?} => 3,\n")
    } else {
        String::new()
    };
    writeln!(
        out,
        "pub fn check_uncertainty_edge_cases(\n\
         mut decide: impl FnMut(&{ty}, &{ty}) -> Option<Sign>,\n\
         ) -> [[usize; 2]; 4] {{\n\
         let mut counts = [[0; 2]; 4];\n\
         for (p, e) in uncertainty_edge_cases() {{\n\
         let kind = match e.map(|x| x > 0.0) {{\n\
         [true, false, false] => 0,\n\
         [false, true, false] => 1,\n\
         [false, false, true] => 2,\n\
         {all}\
         _ => panic!(\"e = {{e:?}}\"),\n\
         }};\n\
         // Powers of two, far from the subnormals: halving them is exact.\n\
         let half = e.map(|x| x / 2.0);\n\
         assert_eq!(half.map(|x| x * 2.0), e);\n\
         let err = [half; {points}];\n\
         let Some(sign) = decide(&p, &err) else {{\n\
         counts[kind][1] += 1;\n\
         continue;\n\
         }};\n\
         assert_ne!(sign, Sign::Zero, \"{{p:?}} e = {{e:?}}\");"
    )
    .unwrap();
    if sampled {
        writeln!(
            out,
            "// Every coordinate at its lowest or at its highest, but the one of\n\
             // `flip`, at the other end.\n\
             let corner = |high: bool, flip: Option<(usize, usize)>| -> [[Dyadic; 3]; {points}] {{\n\
             std::array::from_fn(|i| {{\n\
             std::array::from_fn(|k| {{\n\
             let (x, d) = (Dyadic::from_f64(p[i][k]), Dyadic::from_f64(err[i][k]));\n\
             if high != (flip == Some((i, k))) {{ &x + &d }} else {{ &x - &d }}\n\
             }})\n\
             }})\n\
             }};\n\
             for high in [false, true] {{\n\
             let flips = (0..{points}).flat_map(|i| (0..3).map(move |k| Some((i, k))));\n\
             for flip in std::iter::once(None).chain(flips) {{\n\
             assert_eq!(exact_at(&corner(high, flip)), sign, \"{{p:?}} e = {{e:?}}, corner {{high}} {{flip:?}}\");\n\
             }}\n\
             }}"
        )
        .unwrap();
    } else {
        writeln!(
            out,
            "for corner in 0..1u64 << (3 * {points}) {{\n\
             let c: [[Dyadic; 3]; {points}] = std::array::from_fn(|i| {{\n\
             std::array::from_fn(|k| {{\n\
             let (x, d) = (Dyadic::from_f64(p[i][k]), Dyadic::from_f64(err[i][k]));\n\
             if (corner >> (3 * i + k)) & 1 == 0 {{ &x - &d }} else {{ &x + &d }}\n\
             }})\n\
             }});\n\
             assert_eq!(exact_at(&c), sign, \"{{p:?}} e = {{e:?}}, corner {{corner}}\");\n\
             }}"
        )
        .unwrap();
    }
    out.push_str("counts[kind][0] += 1;\n}\ncounts\n}\n");
}
