//! # yeild
//!
//! generator yield implmentation
//!
use std::any::{Any, TypeId};

use no_drop::NoDrop;
use gen_impl::Generator;
use rt::{Context, ContextStack, Error};

/// it's a special return instruction that yield nothing
/// but only terminate the generator safely
#[macro_export]
macro_rules! done { () => ({ return $crate::done() }) }

/// don't use it directly, use done!() macro instead
#[inline]
pub fn done<T>() -> T {
    let env = ContextStack::current();
    let context = env.top();
    // set self sp as 0 to indicate it's done
    // it safe to do so because it's not used yet before
    // siwtch to the parent
    context
        .regs
        .set_sp(unsafe { ::std::mem::transmute(0usize) });
    unsafe { ::std::mem::uninitialized() }
}

#[inline]
pub fn raw_yield_now(env: &ContextStack, cur: &mut Context, para: usize) {
    let parent = env.pop_context(cur as *mut _);
    if parent.regs.swap(para) != 0 {
        #[cold]
        panic!(Error::Cancel);
    }
}

/// raw yiled without catch passed in para
#[inline]
fn raw_yield<T: Any>(env: &ContextStack, context: &mut Context, v: T) {
    // check the context
    if !context.is_generator() {
        #[cold]
        panic!("yield from none generator context");
    }

    assert_eq!(TypeId::of::<T>(), context.ret_type);

    let para = NoDrop::new(v);
    raw_yield_now(env, context, para.encode_usize())
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
        #[cold]
        {
            error!("get yield from none generator context");
            panic!(Error::ContextErr);
        }
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

    // TODO: do more checks about cancel
    // check the context, already checked in co_ctx()
    // if !context.is_generator() {
    //     info!("yield from none coroutine context");
    //     // do nothing, just return
    //     return;
    // }

    let parent = env.pop_context(context);
    let para = NoDrop::new(v);
    if parent.regs.swap(para.encode_usize()) != 0 {
        #[cold]
        panic!(Error::Cancel);
    }
}

/// coroutine get passed in yield para
pub fn co_get_yield<A: Any>() -> Option<A> {
    match ContextStack::current().co_ctx() {
        Some(ctx) => ctx.get_para(),
        None => None,
    }
}
