//! # yield
//!
//! generator yield implementation
//!
use std::any::Any;
use std::sync::atomic;

use crate::gen_impl::{unlikely, Generator};
use crate::reg_context::RegContext;
use crate::rt::{is_generator, Context, ContextStack, Error, REF_ACTIVE};

/// it's a special return instruction that yield nothing
/// but only terminate the generator safely
#[macro_export]
macro_rules! done {
    () => {{
        return $crate::done();
    }};
}

/// don't use it directly, use done!() macro instead
/// would panic if use in none generator context
#[doc(hidden)]
#[inline]
pub fn done<T>() -> T {
    assert!(is_generator(), "done is only possible in a generator");
    std::panic::panic_any(Error::Done)
}

/// switch back to parent context
#[inline]
pub fn yield_now() {
    let env = ContextStack::current();
    let cur = env.top();
    raw_yield_now(&env, cur);
}

#[inline]
pub fn raw_yield_now(env: &ContextStack, cur: &mut Context) {
    let parent = env.pop_context(cur as *mut _);
    RegContext::swap(&mut cur.regs, &parent.regs);
}

/// raw yield without catch passed in para
#[inline]
fn raw_yield<T: Any>(env: &ContextStack, context: &mut Context, v: T) {
    // check the context
    if unlikely(!context.is_generator()) {
        panic!("yield from none generator context");
    }

    context.set_ret(v);
    context._ref -= 1;
    raw_yield_now(env, context);

    // here we just panic to exit the func
    if unlikely(context._ref != REF_ACTIVE) {
        std::panic::panic_any(Error::Cancel);
    }
}

/// yield something without catch passed in para
#[inline]
#[deprecated(since = "0.6.18", note = "please use `scope` version instead")]
pub fn yield_with<T: Any>(v: T) {
    let env = ContextStack::current();
    let context = env.top();
    raw_yield(&env, context, v);
}

/// get the passed in para
#[inline]
#[deprecated(since = "0.6.18", note = "please use `scope` version instead")]
pub fn get_yield<A: Any>() -> Option<A> {
    let context = ContextStack::current().top();
    raw_get_yield(context)
}

/// get the passed in para from context
#[inline]
fn raw_get_yield<A: Any>(context: &mut Context) -> Option<A> {
    // check the context
    if unlikely(!context.is_generator()) {
        panic!("get yield from none generator context");
    }

    context.get_para()
}

/// yield and get the send para
/// This function yields a value and returns the next value sent to the generator.
/// A static lifetime is required for the yielded value because the generator may
/// be suspended indefinitely, and we need to refer to it across the context switch
/// without worrying about stack invalidation.
#[inline]
#[deprecated(since = "0.6.18", note = "please use `scope` version instead")]
pub fn yield_<A: Any, T: Any>(v: T) -> Option<A> {
    let env = ContextStack::current();
    let context = env.top();
    raw_yield(&env, context, v);
    atomic::compiler_fence(atomic::Ordering::Acquire);
    raw_get_yield(context)
}

/// `yield_from`
#[deprecated(since = "0.6.18", note = "please use `scope` version instead")]
pub fn yield_from<A: Any, T: Any>(mut g: Generator<A, T>) -> Option<A> {
    let env = ContextStack::current();
    let context = env.top();
    let mut p = context.get_para();
    while unlikely(!g.is_done()) {
        let r = g.raw_send(p)?;
        raw_yield(&env, context, r);
        p = context.get_para();
    }
    drop(g); // explicitly consume g
    p
}

/// Yield a value from a coroutine context, resuming with a sent value.
///
/// Unlike generators, coroutines can yield indefinitely and their parent may
/// send values back. This function properly handles cancellation by panicking
/// if the coroutine has been cancelled while suspended.
pub fn co_yield_with<T: Any>(v: T) {
    let env = ContextStack::current();
    let context = env.co_ctx().unwrap();

    // check the context, already checked in co_ctx()
    // if !context.is_generator() {
    //     info!("yield from none coroutine context");
    //     // do nothing, just return
    //     return;
    // }

    // A body that caught the `Cancel` panic raised after the swap below must
    // not be able to suspend again: `_ref` is still `REF_CANCELLED` here, and
    // decrementing it would turn it into `REF_ACTIVE`, which makes the
    // generator look done while this body remains suspended.
    if unlikely(context._ref != REF_ACTIVE) {
        std::panic::panic_any(Error::Cancel);
    }

    context.set_ret(v);
    context._ref -= 1;

    let parent = env.pop_context(context);
    let top = unsafe { &mut *context.parent };
    // here we should use the top regs
    RegContext::swap(&mut top.regs, &parent.regs);

    // here we just panic to exit the func, like the generator path: `cancel`
    // sets `_ref` to `REF_CANCELLED` while the coroutine is suspended, so the
    // panic fires as soon as it is resumed instead of at the next yield
    if unlikely(context._ref != REF_ACTIVE) {
        std::panic::panic_any(Error::Cancel);
    }
}

/// Get the parameter sent to the current coroutine via `send()`.
///
/// Returns `None` if no value has been sent, or if called outside a coroutine context.
/// This is the coroutine equivalent of `get_yield()` but uses the coroutine's own
/// parameter storage instead of the generic context.
pub fn co_get_yield<A: Any>() -> Option<A> {
    ContextStack::current()
        .co_ctx()
        .and_then(|ctx| ctx.get_para())
}

/// Set a parameter in the current coroutine context for the parent to receive.
///
/// This allows coroutines to set a value that will be returned when the parent
/// resumes after a `co_yield_with()`. Only has effect when called from within
/// a coroutine; silently does nothing if called outside a coroutine context.
pub fn co_set_para<A: Any>(para: A) {
    if let Some(ctx) = ContextStack::current().co_ctx() {
        ctx.set_para(para)
    }
}
