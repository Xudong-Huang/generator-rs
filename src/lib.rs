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
pub use rt::Context;
pub use rt::ContextStack;

mod generator;
pub use generator::FnGenerator;

use std::ptr;
use context::Context as RegContext;

/// generator trait
pub trait Generator<A> {
    /// Output type
    type Output;

    /// send interface
    fn send(&mut self, para: A) -> Self::Output;

    /// is finished
    fn is_done(&self) -> bool;
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
    ($val:expr) => ({
        _yield_!($val);
        generator::ContextStack::current().top().get_para().unwrap()
    });

    () => (_yield!(()));
}

/// yield without get the passed in para
#[macro_export]
macro_rules! _yield_ {
    // `(para)`
    // val: the value that need to be yield
    // and got the send para from context
    ($val:expr) => ({
        let context = generator::ContextStack::current().top();
        let _no_use = context.get_flag();
        context.save();
        if *_no_use {
            *_no_use = false;
            context.set_ret($val);
            // don't use the return instruction
            generator::yield_now();
            // context.load();
            return $val;
        }
    });

    () => (_yield_!(()));
}


