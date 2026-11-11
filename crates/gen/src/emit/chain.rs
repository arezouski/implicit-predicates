//! The function that runs the stages of a predicate in order.
//!
//! The exact stages are tried in the order: differences that are exact, the
//! translated expansion form, then the `#[exact]` form if one is given. The
//! order is the same for every predicate for now.

use std::fmt::Write;

use super::comment;

/// Writes `staged`: the filter for explicit points, then the exact
/// differences, then the translated expansions, then the stage of the
/// `#[exact]` form when `untranslated` is true.
pub(crate) fn write_chain(out: &mut String, points: usize, untranslated: bool) {
    let (last_doc, last_call) = if untranslated {
        (
            ", then [`expansions_untranslated`]",
            "\n.or_else(|| expansions_untranslated(p))",
        )
    } else {
        ("", "")
    };
    comment(
        out,
        "///",
        0,
        &format!(
            "The sign of the predicate for explicit points, from the first stage that \
             decides.\n\n\
             Runs [`filter`] with `e = 0.0`, then [`exact_differences`], then \
             [`expansions_translated`]{last_doc}. `None` means that no stage can decide."
        ),
    );
    writeln!(
        out,
        "#[inline]\n\
         pub fn staged(p: [&[f64; 3]; {points}]) -> Option<Sign> {{\n\
         filter(p, 0.0)\n\
         .or_else(|| exact_differences(p))\n\
         .or_else(|| expansions_translated(p)){last_call}\n\
         }}\n"
    )
    .unwrap();
}
