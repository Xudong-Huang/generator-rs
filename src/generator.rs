//! # generator
//!
//! generator trait
//!

/// generator trait
pub trait Generator<A> {
    /// Output type
    type Output;

    /// raw send
    fn raw_send(&mut self, para: Option<A>) -> Option<Self::Output>;

    /// send interface
    fn send(&mut self, para: A) -> Self::Output {
        let ret = self.raw_send(Some(para));
        ret.unwrap()
    }

    /// cancel generator
    fn cancel(&mut self);

    /// is finished
    fn is_done(&self) -> bool;

    /// get stack total size and used size in word
    fn stack_usage(&self) -> (usize, usize);
}

impl<'a, A, T> Iterator for Generator<A, Output = T> + 'a {
    type Item = T;
    // The 'Iterator' trait only requires the 'next' method to be defined. The
    // return type is 'Option<T>', 'None' is returned when the 'Iterator' is
    // over, otherwise the next value is returned wrapped in 'Some'
    fn next(&mut self) -> Option<T> {
        self.raw_send(None)
    }
}
