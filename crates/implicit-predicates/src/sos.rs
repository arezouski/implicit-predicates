//! `insphere` with its ties broken by a symbolic perturbation ordered by the
//! indices of the points.
//!
//! The sign of `insphere` is that of the 5x5 determinant with rows
//! `(x, y, z, x^2 + y^2 + z^2, 1)`. The perturbation subtracts from the
//! lifted coordinate of each point `eps^r`, where `r` is the rank of its
//! index among the five (1 for the smallest) and `eps` is positive and
//! smaller than anything the coordinates can tell apart. The determinant is
//! linear in the lifted column, so its perturbed value is the unperturbed one
//! minus `eps^r` times the cofactor of that entry, summed over the points.
//! The cofactor of the lifted entry of row `j` (counting from 0) is
//! `(-1)^(j + 1)` times the 4x4 determinant of the rows `(x, y, z, 1)` of the
//! other four points, which is `orient3d` of those four in their order. So
//! when the determinant is zero its perturbed sign is that of the first
//! `orient3d` that is not zero, taking the points in the order of their
//! indices and omitting each one in turn, negated when the point omitted is
//! at an odd position of the arguments.

#[cfg(doc)]
use crate::Weight;
use crate::{Point, Scratch, ScratchExhausted, Sign, insphere_in, orient3d_in};
#[cfg(feature = "std")]
use crate::{insphere, orient3d};

/// [`insphere`] with its ties broken: the sign is never [`Sign::Zero`]
/// unless the five points are coplanar.
///
/// `idx[k]` is an index of the point given in position `k` (`a` is 0, `e`
/// is 4), such as its position in the list of points of a mesh. The five
/// indices must be distinct; only their order matters. When `insphere` is
/// not zero, its sign is returned. When it is zero, the sign is that of a
/// symbolic perturbation: the lifted coordinate `x^2 + y^2 + z^2` of each
/// point, in the 5x5 determinant of [`insphere`], is decreased by an
/// infinitesimal, far larger for a smaller index than for any larger one.
/// So when `orient3d(a, b, c, d)` is positive and `e` is on the sphere
/// through the other four, `e` counts as inside it if its index is the
/// smallest of the five. With a smaller index on another point, `e` may
/// count as inside or outside, depending on where the points are. Every caller
/// that gives each point the same index wherever it appears gets ties
/// broken consistently, as a Delaunay tetrahedralization needs.
///
/// The perturbation leaves no tie when four of the five points are not
/// coplanar, even if two of the points are the same: the result is
/// [`Sign::Zero`] exactly when the five points are coplanar. Then they lie
/// on no sphere, and `insphere` is zero too. As for [`insphere`], the sign
/// means inside or outside only when `orient3d(a, b, c, d)` is positive, and
/// the opposite when it is negative; the function does not check it.
/// Permuting the points together with their indices multiplies the sign by
/// the parity of the permutation.
///
/// A tie is broken by up to five calls to [`orient3d`], each on four of the
/// points, in the order of the indices of the point left out; one of the
/// first four is not zero whenever the points are not coplanar.
///
/// Requires the `std` feature, which is on by default.
///
/// # Panics
///
/// - In the cases where [`insphere`] or [`orient3d`] panic: an input of a
///   point is not finite, or the exact coordinates of a point have a weight
///   other than [`Weight::One`]; and, in debug builds, a point that breaks
///   the contract of [`Point`], as they say.
/// - In debug builds, if two of the indices are equal. In release builds the
///   sign is then unspecified, but it is that of `insphere` whenever
///   `insphere` is not zero, and there is no panic.
///
/// # Examples
///
/// ```
/// use implicit_predicates::{Sign, insphere, insphere_sos, orient3d};
///
/// // Five points of the unit sphere; the first four positively oriented.
/// let (a, b, c, d, e) = (
///     [1.0, 0.0, 0.0],
///     [0.0, 1.0, 0.0],
///     [0.0, 0.0, 1.0],
///     [-1.0, 0.0, 0.0],
///     [0.0, 0.0, -1.0],
/// );
/// assert_eq!(orient3d(&a, &b, &c, &d), Sign::Positive);
/// assert_eq!(insphere(&a, &b, &c, &d, &e), Sign::Zero);
/// // `e` has the smallest index: it counts as inside.
/// assert_eq!(insphere_sos(&a, &b, &c, &d, &e, [1, 2, 3, 4, 0]), Sign::Positive);
/// // `a` has the smallest index: here `e` counts as outside, but with `e`
/// // elsewhere on the sphere it could count as inside.
/// assert_eq!(insphere_sos(&a, &b, &c, &d, &e, [0, 1, 2, 3, 4]), Sign::Negative);
/// // The same point twice is a tie too, broken the same way.
/// assert_eq!(insphere_sos(&a, &b, &c, &d, &a, [1, 2, 3, 4, 0]), Sign::Positive);
/// // Five coplanar points stay a tie.
/// let f = [0.0, -1.0, 0.0];
/// assert_eq!(insphere_sos(&a, &b, &d, &f, &[0.5, 0.5, 0.0], [0, 1, 2, 3, 4]), Sign::Zero);
/// ```
#[cfg(feature = "std")]
#[inline]
pub fn insphere_sos(
    a: &impl Point,
    b: &impl Point,
    c: &impl Point,
    d: &impl Point,
    e: &impl Point,
    idx: [usize; 5],
) -> Sign {
    check_indices(&idx);
    let sign = insphere(a, b, c, d, e);
    if sign != Sign::Zero {
        return sign;
    }
    tie_in_thread_arena(a, b, c, d, e, &idx)
}

/// `insphere_sos` with the arena `scratch` instead of the arena of the
/// thread: the same sign, or an error if `scratch` runs out of room.
/// Available without `std`.
///
/// `scratch` is left as it was, on success and on error. The arena is used
/// as [`insphere_in`] and [`orient3d_in`] use it, one call after another, so
/// an arena that suffices for each of them suffices for this one.
///
/// # Errors
///
/// [`ScratchExhausted`] if `scratch` runs out of room; a larger arena then
/// gives the result.
///
/// # Panics
///
/// If an input of a point is not finite, or the exact coordinates of a
/// point have a weight other than [`Weight::One`]; and, in debug builds, if
/// two of the indices are equal, or a point breaks the contract of [`Point`]
/// in one of the ways that [`insphere_in`] and [`orient3d_in`] check. After
/// a panic `scratch` may be left above the top it had; resetting it to a
/// mark taken before the call releases what the call wrote.
///
/// # Examples
///
/// ```
/// use implicit_predicates::{Scratch, Sign, insphere_sos_in};
///
/// let (a, b, c, d, e) = (
///     [1.0, 0.0, 0.0],
///     [0.0, 1.0, 0.0],
///     [0.0, 0.0, 1.0],
///     [-1.0, 0.0, 0.0],
///     [0.0, 0.0, -1.0],
/// );
/// let mut buf = vec![0.0; 6835];
/// let mut scratch = Scratch::borrowed(&mut buf);
/// let sign = insphere_sos_in(&mut scratch, &a, &b, &c, &d, &e, [1, 2, 3, 4, 0]);
/// assert_eq!(sign, Ok(Sign::Positive));
/// assert_eq!(scratch.used(), 0);
///
/// // An arena too small is reported.
/// let mut small = [0.0; 8];
/// let mut scratch = Scratch::borrowed(&mut small);
/// assert!(insphere_sos_in(&mut scratch, &a, &b, &c, &d, &e, [1, 2, 3, 4, 0]).is_err());
/// ```
#[inline]
pub fn insphere_sos_in(
    scratch: &mut Scratch<'_>,
    a: &impl Point,
    b: &impl Point,
    c: &impl Point,
    d: &impl Point,
    e: &impl Point,
    idx: [usize; 5],
) -> Result<Sign, ScratchExhausted> {
    check_indices(&idx);
    let sign = insphere_in(scratch, a, b, c, d, e)?;
    if sign != Sign::Zero {
        return Ok(sign);
    }
    tie_in(scratch, a, b, c, d, e, &idx)
}

/// Whether the five indices are distinct.
fn distinct(idx: &[usize; 5]) -> bool {
    (0..5).all(|i| (i + 1..5).all(|j| idx[i] != idx[j]))
}

/// The message of the debug assertion on the indices.
const DISTINCT: &str = "the five indices of `insphere_sos` must be distinct";

/// The perturbed sign of a tie of `insphere`, from `minor(j)`, the sign of
/// `orient3d` of the points other than the argument `j`, in the order of the
/// arguments. `minor` is called on the arguments in the order of their
/// indices until one gives a sign that is not zero.
#[inline(always)]
fn break_tie<E>(
    idx: &[usize; 5],
    mut minor: impl FnMut(usize) -> Result<Sign, E>,
) -> Result<Sign, E> {
    let mut by_index = [0, 1, 2, 3, 4];
    // Comparisons only: no arithmetic on the indices, which may be any
    // `usize`.
    by_index.sort_unstable_by_key(|&j| idx[j]);
    for j in by_index {
        let sign = minor(j)?;
        if sign != Sign::Zero {
            return Ok(if j % 2 == 1 { -sign } else { sign });
        }
    }
    Ok(Sign::Zero)
}

/// The tie-break of [`insphere_sos`], with the arena of
/// the thread.
#[cfg(feature = "std")]
#[cold]
#[inline(never)]
fn tie_in_thread_arena(
    a: &impl Point,
    b: &impl Point,
    c: &impl Point,
    d: &impl Point,
    e: &impl Point,
    idx: &[usize; 5],
) -> Sign {
    let sign = break_tie(idx, |j| {
        Ok::<_, core::convert::Infallible>(match j {
            0 => orient3d(b, c, d, e),
            1 => orient3d(a, c, d, e),
            2 => orient3d(a, b, d, e),
            3 => orient3d(a, b, c, e),
            _ => orient3d(a, b, c, d),
        })
    });
    match sign {
        Ok(sign) => sign,
        Err(never) => match never {},
    }
}

/// The tie-break of [`insphere_sos_in`], with the
/// arena `scratch`, which every `orient3d_in` leaves as it was.
#[cold]
#[inline(never)]
fn tie_in(
    scratch: &mut Scratch<'_>,
    a: &impl Point,
    b: &impl Point,
    c: &impl Point,
    d: &impl Point,
    e: &impl Point,
    idx: &[usize; 5],
) -> Result<Sign, ScratchExhausted> {
    break_tie(idx, |j| match j {
        0 => orient3d_in(scratch, b, c, d, e),
        1 => orient3d_in(scratch, a, c, d, e),
        2 => orient3d_in(scratch, a, b, d, e),
        3 => orient3d_in(scratch, a, b, c, e),
        _ => orient3d_in(scratch, a, b, c, d),
    })
}

/// Checks, in debug builds, that the indices are distinct.
#[inline(always)]
fn check_indices(idx: &[usize; 5]) {
    debug_assert!(distinct(idx), "{DISTINCT}: {idx:?}");
}
