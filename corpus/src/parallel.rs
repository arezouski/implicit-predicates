//! Running a task over many items on several threads, with the results
//! taken in the order of the items.

use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;

/// The number of threads to use when none is asked for: one per hardware
/// thread available to the process.
pub fn default_jobs() -> usize {
    std::thread::available_parallelism().map_or(1, usize::from)
}

/// Applies `work` to every item on `jobs` threads (at least one) and passes
/// the results to `each` on the calling thread, in the order of the items,
/// whichever thread finishes first.
///
/// `each` stops the run by returning [`ControlFlow::Break`]: no further
/// result is passed, the items not yet started are left alone, and the value
/// is returned. Returns `None` when every result was taken.
///
/// # Panics
///
/// Panics if `work` panics on any item.
pub fn map_ordered<T, R, B>(
    items: &[T],
    jobs: usize,
    work: impl Fn(&T) -> R + Sync,
    mut each: impl FnMut(R) -> ControlFlow<B>,
) -> Option<B>
where
    T: Sync,
    R: Send,
{
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..jobs.clamp(1, items.len().max(1)) {
            let sender = sender.clone();
            let (next, stop, work) = (&next, &stop, &work);
            scope.spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else { break };
                    if sender.send((index, work(item))).is_err() {
                        break;
                    }
                }
            });
        }
        // The receiver ends when every thread has dropped its sender.
        drop(sender);
        let mut waiting = BTreeMap::new();
        let mut expected = 0;
        for (index, result) in receiver {
            waiting.insert(index, result);
            while let Some(result) = waiting.remove(&expected) {
                expected += 1;
                if let ControlFlow::Break(value) = each(result) {
                    stop.store(true, Ordering::Relaxed);
                    return Some(value);
                }
            }
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Later items finish first, so the results arrive out of order.
    fn slow_first(x: &u64) -> u64 {
        std::thread::sleep(std::time::Duration::from_millis(40u64.saturating_sub(*x)));
        x * x
    }

    #[test]
    fn results_come_in_the_order_of_the_items_with_any_number_of_threads() {
        let items: Vec<u64> = (0..40).collect();
        let expected: Vec<u64> = items.iter().map(|x| x * x).collect();
        for jobs in [0, 1, 3, 16, 100] {
            let mut seen = Vec::new();
            let stopped = map_ordered(&items, jobs, slow_first, |r| {
                seen.push(r);
                ControlFlow::<()>::Continue(())
            });
            assert_eq!((stopped, &seen), (None, &expected), "{jobs} threads");
        }
    }

    #[test]
    fn a_break_stops_the_run_and_returns_its_value() {
        let items: Vec<u64> = (0..1000).collect();
        let started = AtomicUsize::new(0);
        let mut seen = Vec::new();
        let stopped = map_ordered(
            &items,
            4,
            |x| {
                started.fetch_add(1, Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(1));
                *x
            },
            |r| {
                seen.push(r);
                if r == 5 {
                    ControlFlow::Break("five")
                } else {
                    ControlFlow::Continue(())
                }
            },
        );
        assert_eq!(stopped, Some("five"));
        assert_eq!(seen, [0, 1, 2, 3, 4, 5]);
        assert!(started.load(Ordering::Relaxed) < items.len());
    }

    #[test]
    fn no_item_gives_no_result() {
        let none: [u64; 0] = [];
        let stopped = map_ordered(&none, 4, |x| *x, |_| ControlFlow::Break(()));
        assert_eq!(stopped, None);
    }

    #[test]
    #[should_panic]
    fn a_panic_in_the_work_is_not_swallowed() {
        let items = [1, 2, 3];
        map_ordered(
            &items,
            2,
            |x| assert_ne!(*x, 2),
            |()| ControlFlow::<()>::Continue(()),
        );
    }
}
