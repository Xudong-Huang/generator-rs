//! # yeild
//!
//! generator yield implmentation
//!

use std::ptr;
use std::any::Any;
use rt::{Context, ContextStack};
use context::Context as RegContext;

/// yield error types
#[allow(dead_code)]
pub enum Error {
    Cancel,
    StackErr,
    ContextErr,
}

/// switch back to parent context
#[inline]
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
fn raw_yield<T: Any>(context: &mut Context, v: T) {
    // check the context
    if !context.is_generator() {
        panic!(Error::ContextErr);
    }

    let _no_use = context.get_flag();
    context.save();
    if *_no_use {
        *_no_use = false;
        context.set_ret(v);
        yield_now();
    }

    // here we just panic to exit the func
    if context._ref != 1 {
        panic!(Error::Cancel);
    }
}

/// yiled something without catch passed in para
#[inline]
pub fn yield_with<T: Any>(v: T) {
    raw_yield(ContextStack::current().top(), v); 
}

/// yiled with something and return the passed in para
#[inline]
pub fn get_yield<A: Any, T: Any>(v: T) -> Option<A> {
    let context = ContextStack::current().top();
    raw_yield(context, v);
    context.get_para()
}

/// yiled and get the send para
#[macro_export]
macro_rules! _yield {
    // `(para)`
    // val: the value that need to be yield
    // and got the send para from context
    ($val:expr) => (generator::get_yield($val).unwrap());

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

