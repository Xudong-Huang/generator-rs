#[derive(Debug)]
pub struct RawVec<T> {
    buf: Vec<T>,
}

impl<T> RawVec<T> {
    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        RawVec {
            buf: Vec::with_capacity(cap),
        }
    }

    #[inline]
    pub fn ptr(&self) -> *mut T {
        let ptr = self.buf.as_ptr();
        ptr as *mut T
    }

    #[inline]
    pub fn cap(&self) -> usize {
        self.buf.capacity()
    }
}
