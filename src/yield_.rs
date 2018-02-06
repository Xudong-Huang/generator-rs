//! # yeild
//!
//! generator yield implmentation
//!

use std::any::Any;
use stack::StackPointer;
use gen_impl::Generator;
use reg_context::RegContext;
use rt::{Context, ContextStack, Error};

/// it's a special return instruction that yield nothing
/// but only terminate the generator safely
#[macro_export]
macro_rules! done { () => ({ return $crate::done() }) }

/// don't use it directly, use done!() macro instead
#[inline]
pub fn done<T>() -> T {
    // set the done bit for this special return
    ContextStack::current().top()._ref = 0xf;
    unsafe { ::std::mem::uninitialized() }
}

/// switch back to parent context
#[inline]
pub fn yield_now(sp: StackPointer) {
    let env = ContextStack::current();
    let cur = env.top();
    let parent = unsafe { &mut *cur.parent };
    parent.regs.set_sp(sp);
    raw_yield_now(&env, cur);
}

#[inline]
pub fn raw_yield_now(env: &ContextStack, cur: &mut Context) {
    let parent = env.pop_context(cur as *mut _);
    RegContext::swap(&mut cur.regs, &mut parent.regs, 0);
}

/// raw yiled without catch passed in para
#[inline]
fn raw_yield<T: Any>(env: &ContextStack, context: &mut Context, v: T) {
    // check the context
    if !context.is_generator() {
        panic!("yield from none generator context");
    }

    context.set_ret(v);
    context._ref -= 1;
    raw_yield_now(env, context);

    // here we just panic to exit the func
    if context._ref != 1 {
        panic!(Error::Cancel);
    }
}

/// yiled something without catch passed in para
#[inline]
// #[deprecated(since="0.5.0", note="please use `scope` instead")]
pub fn yield_with<T: Any>(v: T) {
    let env = ContextStack::current();
    let context = env.top();
    raw_yield(&env, context, v);
}

/// get the passed in para
#[inline]
// #[deprecated(since="0.5.0", note="please use `scope` instead")]
pub fn get_yield<A: Any>() -> Option<A> {
    let context = ContextStack::current().top();
    raw_get_yield(context)
}

/// get the passed in para from context
#[inline]
fn raw_get_yield<A: Any>(context: &mut Context) -> Option<A> {
    // check the context
    if !context.is_generator() {
        error!("get yield from none generator context");
        panic!(Error::ContextErr);
    }

    context.get_para()
}

/// yiled and get the send para
// here yield need to return a static lifttime value, which is Any required
// this is fine, but it's totally safe that we can refer to the function block
// since we will come back later
#[inline]
// #[deprecated(since="0.5.0", note="please use `scope` instead")]
pub fn yield_<A: Any, T: Any>(v: T) -> Option<A> {
    let env = ContextStack::current();
    let context = env.top();
    raw_yield(&env, context, v);
    raw_get_yield(context)
}

/// `yiled_from`
// #[deprecated(since="0.5.0", note="please use `scope` instead")]
pub fn yield_from<A: Any, T: Any>(mut g: Generator<A, T>) -> Option<A> {
    let env = ContextStack::current();
    let context = env.top();
    let mut p = context.get_para();
    while !g.is_done() {
        match g.raw_send(p) {
            None => return None,
            Some(r) => raw_yield(&env, context, r),
        }
        p = context.get_para();
    }
    p
}

/// coroutine yield
pub fn co_yield_with<T: Any>(v: T) {
    let env = ContextStack::current();
    let context = env.co_ctx().unwrap();

    // check the context, already checked in co_ctx()
    // if !context.is_generator() {
    //     info!("yield from none coroutine context");
    //     // do nothing, just return
    //     return;
    // }

    // here we just panic to exit the func
    if context._ref != 1 {
        panic!(Error::Cancel);
    }

    context.set_ret(v);
    context._ref -= 1;

    let parent = env.pop_context(context);
    let top = unsafe { &mut *context.parent };
    // here we should use the top regs
    RegContext::swap(&mut top.regs, &mut parent.regs, 0);
}

/// coroutine get passed in yield para
pub fn co_get_yield<A: Any>() -> Option<A> {
    match ContextStack::current().co_ctx() {
        Some(ctx) => ctx.get_para(),
        None => None,
    }
}
