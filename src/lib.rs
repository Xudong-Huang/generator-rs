//! # generator
//!
//! Rust generator library
//!

#![feature(fnbox)]
#![feature(rustc_private)]
#![feature(rt)]
#![feature(box_raw)]
#![cfg_attr(test, deny(warnings))]
#![deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate libc;
extern crate context;

mod rt;
use rt::ContextStack;

mod fn_gen;
pub use fn_gen::FnGenerator;

use std::ptr;
use std::any::Any;
use context::Context as RegContext;

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

/// switch back to parent context
pub fn yield_now() {
    let env = ContextStack::current();
    let ctx = env.top();
    let sp = ctx.stack.start();
    // judge if this is root context
    if sp != ptr::null() {
        env.pop();
        let ctx = env.top();
        RegContext::load(&ctx.regs);
    }
}

/// yiled something without catch passed in para
#[inline]
pub fn yield_with<T: Any>(v: T) {
    let context = ContextStack::current().top();
    let _no_use = context.get_flag();
    context.save();
    if *_no_use {
        *_no_use = false;
        context.set_ret(v);
        yield_now();
    }
}

/// yiled with something and return the passed in para
#[inline]
pub fn get_yield_with<A: Any, T: Any>(v: T) -> Option<A> {
    let context = ContextStack::current().top();
    let _no_use = context.get_flag();
    context.save();
    if *_no_use {
        *_no_use = false;
        context.set_ret(v);
        yield_now();
    }
    ContextStack::current().top().get_para()
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

/// yiled and get the send para
#[macro_export]
macro_rules! _yield {
    // `(para)`
    // val: the value that need to be yield
    // and got the send para from context
    ($val:expr) => (generator::get_yield_with($val).unwrap());

    () => (_yield!(()));
}

/// yield without get the passed in para
#[macro_export]
macro_rules! _yield_ {
    // `(para)`
    // val: the value that need to be yield
    // and got the send para from context
    ($val:expr) => (generator::yield_with($val));

    () => (_yield_!(()));
}

