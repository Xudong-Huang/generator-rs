//! # generator
//!
//! generator trait
//!

use std::any::Any;
use fn_gen::FnGenerator;

// default stack size is 1k * sizeof(usize)
const DEFAULT_STACK_SIZE: usize = 1024;

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
}

#[allow(dead_code)]
/// Generator helper
/// this is equal with FnGenerator::<A, _, _>
/// but save some typing
pub struct Gn<A> {
    dummy: A,
}

impl <A: Any> Gn<A> {
    /// create a new generator with default stack size
    pub fn new<'a, T: Any, F>(f: F) -> Box<Generator<A, Output = T> + 'a>
        where F: FnOnce() -> T + 'a
    {
        FnGenerator::<A, T, F>::new_opt(f, DEFAULT_STACK_SIZE)
    }

    /// create a new generator with specified stack size
    pub fn new_opt<'a, T: Any, F>(f: F, size: usize) -> Box<Generator<A, Output = T> + 'a>
        where F: FnOnce() -> T + 'a
    {
        FnGenerator::<A, T, F>::new_opt(f, size)
    }
}

impl<'a, A, T> Iterator for Generator<A, Output=T> + 'a {
    type Item = T;
    // The 'Iterator' trait only requires the 'next' method to be defined. The
    // return type is 'Option<T>', 'None' is returned when the 'Iterator' is
    // over, otherwise the next value is returned wrapped in 'Some'
    fn next(&mut self) -> Option<T> {
        self.raw_send(None)
    }
}

/// create generator
#[macro_export]
macro_rules! generator {
    // `(func, <type>)`
    // func: the expression for unsafe async function which contains yiled
    // para: default send para type to the generator
    ($func:expr, <$para:ty>) => (
        generator::Gn::<$para>::new(move|| {$func})
    );

    ($func:expr) => (generator!($func, <()>));
}
