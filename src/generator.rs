//! # generator
//!
//! generator trait
//!

use std::fmt;
use std::intrinsics::type_name;

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

impl<A, T> fmt::Debug for Box<Generator<A, Output = T>> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            write!(f,
                   "Generator<{}, Output={}> {{ ... }}",
                   type_name::<A>(),
                   type_name::<T>())
        }
    }
}
