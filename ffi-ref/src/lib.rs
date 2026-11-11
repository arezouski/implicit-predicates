//! Bindings to two versions of a third-party C++ library of indirect
//! geometric predicates, used as a reference by tests and benchmarks.
//!
//! - [`legacy`]: the predicate library bundled with the CDT mesher, the one
//!   the corpus files were recorded with.
//! - [`current`]: the current Indirect_Predicates library.
//!
//! Both modules have the same API. Points are built in C++ and owned by a
//! `Points` value; predicates take the `PointId`s it returned. Signs are
//! returned in the convention of `implicit_predicates_corpus::exact`,
//! whatever the convention of the library.
//!
//! The libraries are LGPL licensed and are not part of this repository; see
//! the README of this package.

/// Defines `Points`, `PointId` and `from_corpus` for one library version,
/// given its [`sys::Version`] and the factors that turn its orient3d and
/// insphere signs into ours.
macro_rules! reference_points {
    ($version:ty, $orient3d_factor:expr, $insphere_factor:expr) => {
        use implicit_predicates_corpus::{Corpus, PointDef};

        /// Factor from the library's orient3d sign to ours.
        const ORIENT3D_FACTOR: i8 = $orient3d_factor;
        /// Factor from the library's insphere sign to ours.
        const INSPHERE_FACTOR: i8 = $insphere_factor;

        /// Owner of points built in the reference library. Dropping it frees
        /// them all.
        ///
        /// Not `Send`: the library keeps thread-local memory pools.
        pub struct Points {
            handle: crate::sys::Handle<$version>,
        }

        /// A point of a [`Points`]. Using it with another `Points` gives
        /// whatever point has the same index there, or a panic if none does.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct PointId(u32);

        impl Points {
            /// An empty set of points.
            pub fn new() -> Points {
                Points {
                    handle: crate::sys::Handle::new(),
                }
            }

            /// Adds an explicit point.
            ///
            /// # Panics
            ///
            /// Panics if a coordinate is not finite.
            pub fn explicit(&mut self, xyz: [f64; 3]) -> PointId {
                PointId(self.handle.explicit(xyz))
            }

            /// Adds the LNC point `p + t (q - p)`: `t = 0` is `p` and `t = 1`
            /// is `q`.
            ///
            /// # Panics
            ///
            /// Panics if `p` or `q` is not an explicit point of `self`, or if
            /// `t` is not finite.
            pub fn lnc(&mut self, p: PointId, q: PointId, t: f64) -> PointId {
                PointId(self.handle.lnc(p.0, q.0, t))
            }

            /// Sign of `det[a - d; b - d; c - d]`: 1, 0 or -1.
            ///
            /// # Panics
            ///
            /// Panics if a point does not belong to `self`.
            pub fn orient3d(&self, a: PointId, b: PointId, c: PointId, d: PointId) -> i8 {
                ORIENT3D_FACTOR * self.handle.orient3d([a.0, b.0, c.0, d.0])
            }

            /// Sign of the 4x4 determinant whose rows are `(p - e, |p - e|^2)`
            /// for `p` in `a, b, c, d`: 1, 0 or -1.
            ///
            /// # Panics
            ///
            /// Panics if a point does not belong to `self`.
            pub fn insphere(
                &self,
                a: PointId,
                b: PointId,
                c: PointId,
                d: PointId,
                e: PointId,
            ) -> i8 {
                INSPHERE_FACTOR * self.handle.insphere([a.0, b.0, c.0, d.0, e.0])
            }

            /// For the benchmarks of this package only: everything
            /// [`Points::orient3d`] does, through the same C interface,
            /// except evaluating the predicate. Returns 0. Its time is the
            /// cost of the bindings themselves.
            ///
            /// # Panics
            ///
            /// Panics if a point does not belong to `self`.
            #[doc(hidden)]
            pub fn overhead(&self, a: PointId, b: PointId, c: PointId, d: PointId) -> i8 {
                ORIENT3D_FACTOR * self.handle.overhead([a.0, b.0, c.0, d.0])
            }
        }

        impl Default for Points {
            fn default() -> Points {
                Points::new()
            }
        }

        /// Builds every explicit and LNC point of `corpus`. The returned
        /// vector maps each corpus point index to its [`PointId`], or to
        /// `None` for LPI and TPI points.
        pub fn from_corpus(corpus: &Corpus) -> (Points, Vec<Option<PointId>>) {
            let mut points = Points::new();
            let mut ids: Vec<Option<PointId>> = Vec::with_capacity(corpus.points.len());
            for def in &corpus.points {
                let id = match *def {
                    PointDef::Explicit(xyz) => Some(points.explicit(xyz)),
                    PointDef::Lnc { p, q, t } => {
                        let parent = |i: u32| ids[i as usize].expect("LNC parents are explicit");
                        Some(points.lnc(parent(p), parent(q), t))
                    }
                    PointDef::Lpi { .. } | PointDef::Tpi { .. } => None,
                };
                ids.push(id);
            }
            (points, ids)
        }
    };
}

mod sys;

pub mod current;
pub mod legacy;

/// Hooks for the tests of this package only: each one ends the process
/// when the C interface works as intended.
#[doc(hidden)]
pub mod test_hooks {
    pub use crate::sys::TestHook;

    /// Runs a test hook of the legacy library.
    pub fn legacy(hook: TestHook) {
        crate::sys::test_hook::<crate::sys::Legacy>(hook);
    }

    /// Runs a test hook of the current library.
    pub fn current(hook: TestHook) {
        crate::sys::test_hook::<crate::sys::Current>(hook);
    }
}
