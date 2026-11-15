//! The rules on the names and the size of a point type that the parser does
//! not check, because they come from the code emitted for it.

use crate::error::Error;
use crate::point::{MAX_POINT_NODES, PointType};

/// Names a parameter cannot have: the generated type uses them for its
/// fields and functions. Every other name that the generated code uses for
/// a type, a constant or a variant starts with an uppercase letter, which a
/// parameter cannot.
const RESERVED_PARAMS: [&str; 8] = [
    "approx",
    "err",
    "exact",
    "is_explicit",
    "new",
    "new_unchecked",
    "evaluate",
    "write_exact",
];

/// Names a point type cannot have: the generated modules use them, as
/// their own items, items of the core and of the oracle, or names of the
/// prelude. Every other item they use, primitive types and modules among
/// them, starts with a lowercase letter, which a point type cannot.
const RESERVED_TYPES: [&str; 23] = [
    "AnyPoint",
    "Big",
    "BigCoords",
    "PointError",
    "Point",
    "Scratch",
    "ScratchExhausted",
    "ExactCoords",
    "ExactError",
    "Weight",
    "Mark",
    "Span",
    "Dyadic",
    "UNIT",
    "TINY",
    "SUM_OPERAND_LIMIT",
    "Result",
    "Ok",
    "Err",
    "Option",
    "Some",
    "None",
    "From",
];

/// Checks the names of `point` and of its parameters.
pub(crate) fn check_names(point: &PointType, file: &str) -> Result<(), Error> {
    if !point.name.starts_with(|c: char| c.is_ascii_uppercase()) {
        return Err(Error::at(
            file,
            point.position,
            format!(
                "`{}` cannot name a point type: the name of a point type must start with an \
                 uppercase letter, so that it cannot be a primitive type, a module or a \
                 function that the generated code uses",
                point.name
            ),
        ));
    }
    if RESERVED_TYPES.contains(&point.name.as_str()) {
        return Err(Error::at(
            file,
            point.position,
            format!(
                "`{}` cannot name a point type: the generated code uses that name",
                point.name
            ),
        ));
    }
    for param in &point.params {
        if !param.name.starts_with(|c: char| c.is_ascii_lowercase()) {
            return Err(Error::at(
                file,
                param.position,
                format!(
                    "`{}` cannot name a parameter: the name of a parameter must start with a \
                     lowercase letter, so that it cannot be a type, a constant or a variant \
                     that the generated code uses",
                    param.name
                ),
            ));
        }
        if !is_snake_case(&param.name) {
            return Err(Error::at(
                file,
                param.position,
                format!(
                    "`{}` cannot name a parameter: the name of a parameter must be in snake \
                     case, lowercase ASCII letters, digits and underscores without two \
                     underscores in a row before the end, so that the generated code compiles \
                     without `non_snake_case` warnings",
                    param.name
                ),
            ));
        }
        if RESERVED_PARAMS.contains(&param.name.as_str()) {
            return Err(Error::at(
                file,
                param.position,
                format!(
                    "`{}` cannot name a parameter: the generated type uses that name",
                    param.name
                ),
            ));
        }
    }
    Ok(())
}

/// Whether `name`, which starts with a lowercase ASCII letter, is snake case
/// as Rust's `non_snake_case` lint sees it: only lowercase ASCII letters,
/// digits and underscores, and no two underscores in a row once the
/// trailing ones are removed.
fn is_snake_case(name: &str) -> bool {
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.trim_end_matches('_').contains("__")
}

/// Checks that the graph of `point` has at most [`MAX_POINT_NODES`] nodes:
/// the generated code spends a statement or two on each, in one function,
/// and no limit on roundings bounds a run of exact operations such as
/// negations.
pub(crate) fn check_size(point: &PointType, file: &str) -> Result<(), Error> {
    let nodes = point.graph.nodes.len();
    if nodes > MAX_POINT_NODES {
        return Err(Error::at(
            file,
            point.position,
            format!(
                "the coordinates of `{}` take {nodes} distinct operations and inputs, more \
                 than the {MAX_POINT_NODES} that the generated code supports",
                point.name
            ),
        ));
    }
    Ok(())
}
