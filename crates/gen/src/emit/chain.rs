//! The function that runs the stages of a predicate in order.

use std::fmt::Write;

use super::comment;

/// Writes `staged`: the filter for explicit points, then the exact
/// differences, then `last`, the stage of the form used for exact
/// evaluation.
pub(crate) fn write_chain(out: &mut String, points: usize, last: &str) {
    comment(
        out,
        "///",
        0,
        &format!(
            "The sign of the predicate for explicit points, from the first stage that \
             decides.\n\n\
             Runs [`filter`] with `e = 0.0`, then [`exact_differences`], then \
             [`{last}`]. `None` means that no stage can decide."
        ),
    );
    writeln!(
        out,
        "#[inline]\n\
         pub fn staged(p: [&[f64; 3]; {points}]) -> Option<Sign> {{\n\
         filter(p, 0.0)\n\
         .or_else(|| exact_differences(p))\n\
         .or_else(|| {last}(p))\n\
         }}\n"
    )
    .unwrap();
}
