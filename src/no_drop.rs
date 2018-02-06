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
}

// Try to pack a value into a usize if it fits, otherwise pass its address as a usize.
pub unsafe fn encode_usize<T>(val: &NoDrop<T>) -> usize {
    if mem::size_of::<T>() <= mem::size_of::<usize>()
        && mem::align_of::<T>() <= mem::align_of::<usize>()
    {
        let mut out = 0;
        ptr::copy_nonoverlapping(&val.inner, &mut out as *mut usize as *mut T, 1);
        out
    } else {
        &val.inner as *const T as usize
    }
}

// Unpack a usize produced by encode_usize.
pub unsafe fn decode_usize<T>(val: usize) -> T {
    if mem::size_of::<T>() <= mem::size_of::<usize>()
        && mem::align_of::<T>() <= mem::align_of::<usize>()
    {
        ptr::read(&val as *const usize as *const T)
    } else {
        ptr::read(val as *const T)
    }
}
