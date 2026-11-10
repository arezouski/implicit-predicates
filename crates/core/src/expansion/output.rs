//! The output buffer the expansion functions write their results to.

/// The output buffer of an operation, filled from the start with the nonzero
/// components pushed to it.
pub(super) struct Output<'a> {
    buffer: &'a mut [f64],
    len: usize,
}

impl<'a> Output<'a> {
    /// Wraps `buffer`, checking in debug builds that it holds `capacity`
    /// components.
    pub(super) fn new(buffer: &'a mut [f64], capacity: usize) -> Self {
        debug_assert!(
            buffer.len() >= capacity,
            "output buffer holds {} components but the capacity required is {capacity}",
            buffer.len()
        );
        Output { buffer, len: 0 }
    }

    /// Appends `component` unless it is zero (zero elimination). Panics if
    /// the buffer is full.
    pub(super) fn push(&mut self, component: f64) {
        if component != 0.0 {
            self.buffer[self.len] = component;
            self.len += 1;
        }
    }

    /// The number of components written.
    pub(super) fn finish(self) -> usize {
        self.len
    }
}
