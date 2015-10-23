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
        generator::FnGenerator::<$para, _>::new(move|| {$func})
    );

    ($func:expr) => (generator!($func, <()>));
}

