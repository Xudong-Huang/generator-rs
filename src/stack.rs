//! # generator stack
//!
//!

use std::ptr;
use alloc::raw_vec::RawVec;

/// generator stack
pub struct Stack {
    buf: RawVec<usize>,
}

impl Stack {
    /// Allocate a new stack of `size`. If size = 0, this will fail. Use
    /// `dummy_stack` if you want a zero-sized stack.
    pub fn new(size: usize) -> Stack {
        let stk = Stack { buf: RawVec::with_capacity(size) };
        unsafe {
            let buf = stk.buf.ptr();
            ptr::write_bytes(buf, 0xEE, size);
        }
        stk
    }

    /// get used stack size
    pub fn get_used_size(&self) -> usize {
        let mut offset: usize = 0;
        unsafe {
            let mut magic: usize = 0xEE;
            ptr::write_bytes(&mut magic, 0xEE, 1);
            let mut ptr = self.buf.ptr();
            while *ptr == magic {
                offset += 1;
                ptr = ptr.offset(1);
            }

        }
        self.buf.cap() - offset

    }

    /// get the stack cap
    pub fn size(&self) -> usize {
        self.buf.cap()
    }

    /// Point to the high end of the allocated stack
    pub fn end(&self) -> *mut usize {
        unsafe { self.buf.ptr().offset(self.buf.cap() as isize) as *mut usize }
    }
}
