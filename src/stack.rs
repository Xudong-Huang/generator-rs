//! # generator stack
//!
//!

use alloc::raw_vec::RawVec;

/// generator stack
pub struct Stack {
    buf: RawVec<usize>,
}

impl Stack {
    /// Allocate a new stack of `size`. If size = 0, this will fail. Use
    /// `dummy_stack` if you want a zero-sized stack.
    pub fn new(size: usize) -> Stack {
        Stack { buf: RawVec::with_capacity(size) }
    }

    /// judge if the stack is empty
    pub fn is_empty(&self) -> bool {
        self.buf.cap() == 0
    }

    /// Point to the high end of the allocated stack
    pub fn end(&self) -> *mut usize {
        unsafe { self.buf.ptr().offset(self.buf.cap() as isize) as *mut usize }
    }
}
