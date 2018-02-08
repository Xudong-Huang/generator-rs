use std::{mem, ptr};

// Wrapper to prevent the compiler from automatically dropping a value when it
// goes out of scope. This is particularly useful when dealing with unwinding
// since mem::forget won't be executed when unwinding.
#[allow(unions_with_drop_fields)]
pub union NoDrop<T> {
    inner: T,
}

impl<T> NoDrop<T> {
    pub fn new(t: T) -> Self {
        NoDrop { inner: t }
    }

    // Try to pack a value into a usize if it fits, otherwise pass its address as a usize.
    pub fn encode_usize(&self) -> usize {
        unsafe { &self.inner as *const T as usize }
    }
}

// Unpack a usize produced by encode_usize.
pub unsafe fn decode_usize<T>(val: usize) -> Option<T> {
    if val == 0 {
        #[cold]
        None
    } else {
        let mut v = mem::uninitialized();
        ptr::copy_nonoverlapping(val as *const T, &mut v, 1);
        Some(v)
    }
}
