//! The arena of each thread, for the exact stages of the predicates that do
//! not take one.

use std::cell::RefCell;

use implicit_predicates_core::{Mark, Scratch};

/// The initial size of the arena of each thread, in `f64`: 128 KiB. It grows
/// on the heap when a call needs more.
pub(crate) const INITIAL: usize = 16_384;

/// The arena of a thread, with the mark of its empty state.
struct Arena {
    scratch: Scratch<'static>,
    empty: Mark,
}

thread_local! {
    static ARENA: RefCell<Arena> = RefCell::new({
        let scratch = Scratch::growable(INITIAL);
        let empty = scratch.mark();
        Arena { scratch, empty }
    });
}

/// Runs `f` with an empty arena that grows on the heap: the arena of the
/// thread, created on the first call, or a new one when it is in use (when
/// the exact coordinates of a point call a predicate) or no longer exists
/// (while the thread ends).
///
/// The arena of the thread is emptied before `f` runs, so nothing that an
/// earlier call left in it, even one that panicked, changes the result.
pub(crate) fn with<R>(f: impl FnOnce(&mut Scratch<'_>) -> R) -> R {
    let mut f = Some(f);
    let shared = ARENA.try_with(|arena| {
        let mut arena = arena.try_borrow_mut().ok()?;
        let f = f.take()?;
        let Arena { scratch, empty } = &mut *arena;
        scratch.reset(*empty);
        let result = f(scratch);
        scratch.reset(*empty);
        Some(result)
    });
    match (shared, f) {
        (Ok(Some(result)), _) => result,
        (_, Some(f)) => f(&mut Scratch::growable(INITIAL)),
        // `f` is taken only when it runs on the arena of the thread.
        (_, None) => unreachable!("the arena of the thread ran `f` without a result"),
    }
}
