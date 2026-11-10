//! Declarations of the C interface in `shim/ipref.h` and a safe handle over
//! it. This is the only module with `unsafe` code.

#![allow(unsafe_code)]

use std::ffi::c_int;
use std::marker::PhantomData;
use std::ptr::NonNull;

/// The opaque points handle of either version.
#[repr(C)]
pub(crate) struct RawPoints {
    _opaque: [u8; 0],
}

unsafe extern "C" {
    fn ipref_legacy_points_new() -> *mut RawPoints;
    fn ipref_legacy_test_error_routine();
    fn ipref_legacy_test_return_rounding_upward();
    fn ipref_legacy_test_enter_rounding_upward();
    fn ipref_legacy_test_enter_sse_rounding_upward();
    fn ipref_legacy_points_free(points: *mut RawPoints);
    fn ipref_legacy_explicit(points: *mut RawPoints, x: f64, y: f64, z: f64) -> u32;
    fn ipref_legacy_lnc(points: *mut RawPoints, p: u32, q: u32, t: f64) -> u32;
    fn ipref_legacy_orient3d(points: *const RawPoints, a: u32, b: u32, c: u32, d: u32) -> c_int;
    fn ipref_legacy_insphere(
        points: *const RawPoints,
        a: u32,
        b: u32,
        c: u32,
        d: u32,
        e: u32,
    ) -> c_int;
    fn ipref_legacy_bench_overhead(
        points: *const RawPoints,
        a: u32,
        b: u32,
        c: u32,
        d: u32,
    ) -> c_int;

    fn ipref_current_points_new() -> *mut RawPoints;
    fn ipref_current_test_error_routine();
    fn ipref_current_test_return_rounding_upward();
    fn ipref_current_test_enter_rounding_upward();
    fn ipref_current_test_enter_sse_rounding_upward();
    fn ipref_current_points_free(points: *mut RawPoints);
    fn ipref_current_explicit(points: *mut RawPoints, x: f64, y: f64, z: f64) -> u32;
    fn ipref_current_lnc(points: *mut RawPoints, p: u32, q: u32, t: f64) -> u32;
    fn ipref_current_orient3d(points: *const RawPoints, a: u32, b: u32, c: u32, d: u32) -> c_int;
    fn ipref_current_insphere(
        points: *const RawPoints,
        a: u32,
        b: u32,
        c: u32,
        d: u32,
        e: u32,
    ) -> c_int;
    fn ipref_current_bench_overhead(
        points: *const RawPoints,
        a: u32,
        b: u32,
        c: u32,
        d: u32,
    ) -> c_int;
}

/// The functions of one version of the C interface. Their preconditions are
/// those of `shim/ipref.h`.
pub(crate) struct Api {
    new: unsafe extern "C" fn() -> *mut RawPoints,
    free: unsafe extern "C" fn(*mut RawPoints),
    explicit: unsafe extern "C" fn(*mut RawPoints, f64, f64, f64) -> u32,
    lnc: unsafe extern "C" fn(*mut RawPoints, u32, u32, f64) -> u32,
    orient3d: unsafe extern "C" fn(*const RawPoints, u32, u32, u32, u32) -> c_int,
    insphere: unsafe extern "C" fn(*const RawPoints, u32, u32, u32, u32, u32) -> c_int,
    /// The benchmark hook: an orient3d call without the predicate.
    overhead: unsafe extern "C" fn(*const RawPoints, u32, u32, u32, u32) -> c_int,
    /// The test hooks, in the order of [`TestHook`].
    hooks: [unsafe extern "C" fn(); 4],
}

/// A library version. The functions are constants, so calls through a
/// [`Handle`] are direct calls.
pub(crate) trait Version {
    const API: Api;
}

/// The predicate library bundled with the CDT mesher.
pub(crate) enum Legacy {}

/// The current Indirect_Predicates library.
pub(crate) enum Current {}

impl Version for Legacy {
    const API: Api = Api {
        new: ipref_legacy_points_new,
        free: ipref_legacy_points_free,
        explicit: ipref_legacy_explicit,
        lnc: ipref_legacy_lnc,
        orient3d: ipref_legacy_orient3d,
        insphere: ipref_legacy_insphere,
        overhead: ipref_legacy_bench_overhead,
        hooks: [
            ipref_legacy_test_error_routine,
            ipref_legacy_test_return_rounding_upward,
            ipref_legacy_test_enter_rounding_upward,
            ipref_legacy_test_enter_sse_rounding_upward,
        ],
    };
}

impl Version for Current {
    const API: Api = Api {
        new: ipref_current_points_new,
        free: ipref_current_points_free,
        explicit: ipref_current_explicit,
        lnc: ipref_current_lnc,
        orient3d: ipref_current_orient3d,
        insphere: ipref_current_insphere,
        overhead: ipref_current_bench_overhead,
        hooks: [
            ipref_current_test_error_routine,
            ipref_current_test_return_rounding_upward,
            ipref_current_test_enter_rounding_upward,
            ipref_current_test_enter_sse_rounding_upward,
        ],
    };
}

/// A points handle of version `V` that checks every precondition of the C
/// interface before calling it.
///
/// It holds a raw pointer, so it is neither `Send` nor `Sync`: the reference
/// libraries keep thread-local memory pools, and a handle stays on the
/// thread that created it.
pub(crate) struct Handle<V: Version> {
    raw: NonNull<RawPoints>,
    /// Whether each point, by index, is explicit.
    explicit: Vec<bool>,
    version: PhantomData<V>,
}

impl<V: Version> Handle<V> {
    pub(crate) fn new() -> Self {
        // SAFETY: creating a handle has no precondition.
        let raw = unsafe { (V::API.new)() };
        let raw = NonNull::new(raw).expect("the reference library returned a null handle");
        Handle {
            raw,
            explicit: Vec::new(),
            version: PhantomData,
        }
    }

    fn push(&mut self, index: u32, explicit: bool) -> u32 {
        assert_eq!(
            index as usize,
            self.explicit.len(),
            "unexpected point index"
        );
        self.explicit.push(explicit);
        index
    }

    fn check_room(&self) {
        assert!(self.explicit.len() < u32::MAX as usize, "too many points");
    }

    /// Adds an explicit point and returns its index.
    ///
    /// # Panics
    ///
    /// Panics if a coordinate is not finite.
    pub(crate) fn explicit(&mut self, [x, y, z]: [f64; 3]) -> u32 {
        assert!(
            x.is_finite() && y.is_finite() && z.is_finite(),
            "coordinates must be finite"
        );
        self.check_room();
        // SAFETY: `raw` is a live handle owned by `self`.
        let index = unsafe { (V::API.explicit)(self.raw.as_ptr(), x, y, z) };
        self.push(index, true)
    }

    /// Adds the LNC point `p + t (q - p)` and returns its index.
    ///
    /// # Panics
    ///
    /// Panics if `p` or `q` is not an explicit point of this handle, or if
    /// `t` is not finite.
    pub(crate) fn lnc(&mut self, p: u32, q: u32, t: f64) -> u32 {
        for parent in [p, q] {
            assert!(
                self.explicit.get(parent as usize) == Some(&true),
                "the parents of an LNC point must be explicit points"
            );
        }
        assert!(t.is_finite(), "the LNC parameter must be finite");
        self.check_room();
        // SAFETY: `raw` is a live handle owned by `self`, and `p` and `q`
        // name explicit points of it, as checked above.
        let index = unsafe { (V::API.lnc)(self.raw.as_ptr(), p, q, t) };
        self.push(index, false)
    }

    fn check_points(&self, points: &[u32]) {
        for &point in points {
            assert!(
                (point as usize) < self.explicit.len(),
                "point {point} does not exist"
            );
        }
    }

    /// The library's orient3d sign, in its own convention.
    ///
    /// # Panics
    ///
    /// Panics if a point does not exist or the library returns something
    /// other than -1, 0 or 1.
    pub(crate) fn orient3d(&self, [a, b, c, d]: [u32; 4]) -> i8 {
        self.check_points(&[a, b, c, d]);
        // SAFETY: `raw` is a live handle owned by `self`, and every index
        // names one of its points, as checked above.
        sign(unsafe { (V::API.orient3d)(self.raw.as_ptr(), a, b, c, d) })
    }

    /// The library's insphere sign, in its own convention.
    ///
    /// # Panics
    ///
    /// As [`Handle::orient3d`].
    pub(crate) fn insphere(&self, [a, b, c, d, e]: [u32; 5]) -> i8 {
        self.check_points(&[a, b, c, d, e]);
        // SAFETY: `raw` is a live handle owned by `self`, and every index
        // names one of its points, as checked above.
        sign(unsafe { (V::API.insphere)(self.raw.as_ptr(), a, b, c, d, e) })
    }

    /// The benchmark hook: everything [`Handle::orient3d`] does except
    /// evaluating the predicate; returns 0.
    ///
    /// # Panics
    ///
    /// As [`Handle::orient3d`].
    pub(crate) fn overhead(&self, [a, b, c, d]: [u32; 4]) -> i8 {
        self.check_points(&[a, b, c, d]);
        // SAFETY: `raw` is a live handle owned by `self`, and every index
        // names one of its points, as checked above.
        sign(unsafe { (V::API.overhead)(self.raw.as_ptr(), a, b, c, d) })
    }
}

impl<V: Version> Drop for Handle<V> {
    fn drop(&mut self) {
        // SAFETY: `raw` was created by `V::API.new`, is owned by `self` and
        // is freed only here, once.
        unsafe { (V::API.free)(self.raw.as_ptr()) }
    }
}

/// A test hook of the C interface (see `shim/ipref.h`). None of them
/// returns when the interface works as intended: each ends the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestHook {
    /// Calls the library's error routine from inside a call; the process
    /// must exit with a failure status.
    ErrorRoutine,
    /// Returns from inside a call with the rounding mode set upward; the
    /// process must abort.
    ReturnRoundingUpward,
    /// Sets the rounding mode upward, then creates a points handle; the
    /// process must abort.
    EnterRoundingUpward,
    /// Like [`TestHook::EnterRoundingUpward`], but on x86_64 changes only
    /// the rounding mode of the SSE control register (MXCSR), which Rust's
    /// floating-point arithmetic follows, and not the x87 one.
    EnterSseRoundingUpward,
}

/// Runs a test hook of version `V`.
pub(crate) fn test_hook<V: Version>(hook: TestHook) {
    // SAFETY: the hooks take no arguments and touch no memory of ours.
    // Each ends the process unless the C interface is broken; only then can
    // `ReturnRoundingUpward` come back with the rounding mode set upward.
    // The tests run the hooks in a child process that does nothing else
    // and is judged only by how it ended.
    unsafe { (V::API.hooks[hook as usize])() }
}

fn sign(value: c_int) -> i8 {
    match value {
        -1 => -1,
        0 => 0,
        1 => 1,
        other => panic!("the reference library returned {other}, not a sign"),
    }
}
