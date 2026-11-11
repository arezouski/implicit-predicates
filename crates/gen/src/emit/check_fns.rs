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

/// Writes the check functions for a predicate of `points` points; the one
/// with an uncertainty only when `per_axis`.
pub(crate) fn write_check_fns(out: &mut String, points: usize, per_axis: bool) {
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
    if !per_axis {
        return;
    }
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
             at each of the `2^{}` corners of the boxes of the errors around the \
             approximations; the predicate is affine in each coordinate, so it is then the \
             exact sign at every point within them. Returns how many cases it decided and \
             how many it declined, for `e` not zero on x only, on y only, on z only and on \
             every axis.\n\n\
             # Panics\n\n\
             Panics if `decide` gives a sign that does not hold in the boxes.",
            3 * points
        ),
    );
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
         [true, true, true] => 3,\n\
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
         assert_ne!(sign, Sign::Zero, \"{{p:?}} e = {{e:?}}\");\n\
         for corner in 0..1u64 << (3 * {points}) {{\n\
         let c: [[Dyadic; 3]; {points}] = std::array::from_fn(|i| {{\n\
         std::array::from_fn(|k| {{\n\
         let (x, d) = (Dyadic::from_f64(p[i][k]), Dyadic::from_f64(err[i][k]));\n\
         if (corner >> (3 * i + k)) & 1 == 0 {{ &x - &d }} else {{ &x + &d }}\n\
         }})\n\
         }});\n\
         assert_eq!(exact_at(&c), sign, \"{{p:?}} e = {{e:?}}, corner {{corner}}\");\n\
         }}\n\
         counts[kind][0] += 1;\n\
         }}\n\
         counts\n\
         }}"
    )
    .unwrap();
}
