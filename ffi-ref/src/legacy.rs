//! The predicate library bundled with the CDT mesher, at the commit the
//! corpus files were recorded with.
//!
//! It returns the opposite of our sign for both orient3d and insphere: the
//! factors recorded in `indirect_predicates_corpus::exact`.

use indirect_predicates_corpus::exact::{REFERENCE_INSPHERE_FACTOR, REFERENCE_ORIENT3D_FACTOR};

reference_points!(
    crate::sys::Legacy,
    REFERENCE_ORIENT3D_FACTOR,
    REFERENCE_INSPHERE_FACTOR
);
