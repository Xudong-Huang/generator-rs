//! # generator stack
//!
//!

use std::ptr::{self, NonNull};

#[cfg(nightly)]
use alloc::raw_vec::RawVec;
#[cfg(not(nightly))]
use alloc::RawVec;

#[cfg(windows)]
const MIN_STACK_SIZE: usize = 0x4b0;

#[cfg(unix)]
const MIN_STACK_SIZE: usize = 0x100;

/// generator stack
pub struct Stack {
    buf: RawVec<usize>,
    base: *mut usize,
}

impl Stack {
    pub fn empty() -> Stack {
        Stack {
            buf: RawVec::with_capacity(0),
            base: ptr::null_mut(),
        }
    }

    /// Allocate a new stack of `size`. If size = 0, this is a `dummy_stack`
    pub fn new(size: usize) -> Stack {
        assert_ne!(size, 0);

        let mut size = size;
        // the minimal size
        if size < MIN_STACK_SIZE {
            size = MIN_STACK_SIZE;
        }

        // need to align according to the arch
        let buf: RawVec<usize> = RawVec::with_capacity(size);
        let mut base = unsafe { buf.ptr().offset(buf.cap() as isize) } as usize;
        base = base & !(16 - 1);

        let stk = Stack {
            buf,
            base: base as *mut usize,
        };

        // if size is not even we do the full foot print test
        if (size & 1) == 0 && (size > 8) {
            // we only check the last few words
            size = 8;
        }

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
        let cap = self.buf.cap();
        if cap & 1 != 0 {
            error!("stack size={}, used={}", cap, cap - offset);
        }
        cap - offset
    }

    /// get the stack cap
    #[inline]
    pub fn size(&self) -> usize {
        self.buf.cap()
    }

    /// Point to the high end of the allocated stack
    pub fn end(&self) -> *mut usize {
        self.base
    }

    /// Point to the low end of the allocated stack
    #[allow(dead_code)]
    pub fn begin(&self) -> *mut usize {
        self.buf.ptr()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StackPointer(NonNull<usize>);

impl StackPointer {
    #[inline(always)]
    pub unsafe fn push(&mut self, val: usize) {
        self.0 = NonNull::new_unchecked(self.0.as_ptr().offset(-1));
        *self.0.as_mut() = val;
    }

    #[inline(always)]
    pub unsafe fn new(sp: *mut usize) -> StackPointer {
        StackPointer(NonNull::new_unchecked(sp as *mut usize))
    }

    #[inline(always)]
    pub unsafe fn offset(&self, count: isize) -> *mut usize {
        self.0.as_ptr().offset(count)
    }
}
