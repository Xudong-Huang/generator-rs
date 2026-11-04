//! # yield
//!
//! generator yield implementation
//!

use std::marker::PhantomData;

use crate::gen_impl::Generator;
use crate::rt::{Context, ContextStack, Error, REF_ACTIVE};
use crate::yield_::raw_yield_now;

/// passed in scope type
/// it not use the context to pass data, but keep it's own data ref
/// this struct provide both compile type info and runtime data
pub struct Scope<'scope, 'a, A, T> {
    para: &'a mut Option<A>,
    ret: &'a mut Option<T>,
    scope: PhantomData<&'scope mut &'scope ()>,
}

impl<'a, A, T> Scope<'_, 'a, A, T> {
    /// create a new scope object
    pub(crate) fn new(para: &'a mut Option<A>, ret: &'a mut Option<T>) -> Self {
        Scope {
            para,
            ret,
            scope: PhantomData,
        }
    }

    /// set current generator return value
    #[inline]
    fn set_ret(&mut self, v: T) {
        // use volatile write to prevent compiler optimization reordering
        // *self.ret = Some(v);
        unsafe {
            core::ptr::write_volatile(self.ret, Some(v));
        }
    }

    /// raw yield without catch passed in para
    #[inline]
    fn raw_yield(&mut self, env: &ContextStack, context: &mut Context, v: T) {
        // check the context
        if !context.is_generator() {
            panic!("yield from none generator context");
        }

        self.set_ret(v);
        context._ref -= 1;
        raw_yield_now(env, context);

        // here we just panic to exit the func
        if context._ref != REF_ACTIVE {
            std::panic::panic_any(Error::Cancel);
        }
    }

    /// yield something without catch passed in para
    #[inline]
    pub fn yield_with(&mut self, v: T) {
        let env = ContextStack::current();
        let context = env.top();
        self.raw_yield(&env, context, v);
    }

    /// get current generator send para
    #[inline]
    pub fn get_yield(&mut self) -> Option<A> {
        // in latest nightly (since 2026-08-15) Rust, `Option::take` seems calculated in advance,
        // here we use volatile read to prevent compiler reordering
        // self.para.take()
        unsafe {
            let r = core::ptr::read_volatile(self.para);
            core::ptr::write(self.para, None);
            r
        }
    }

    /// Yield a value and get the sent parameter back, without requiring
    /// `T: 'static`.
    ///
    /// Unlike [`yield_`](Self::yield_), this method is available on a
    /// `Scope<'scope, 'a, A, T>` for any (possibly non-`'static`) `'a`, so `v`
    /// may contain references to data that lives on this generator's own
    /// stack. That extra power is exactly why this method is `unsafe`: the
    /// compiler can no longer verify that those references outlive their use.
    ///
    /// # Safety
    ///
    /// The caller must uphold the borrow rules the type system would normally
    /// enforce for `'a`:
    ///
    /// - Any reference contained in `v` (e.g. borrowed from this generator's
    ///   captured data) must remain valid for as long as the yielded value is
    ///   observable by the `send`/`resume` caller. In practice this means:
    ///   keep the generator alive (do not `drop` it) while such a reference is
    ///   still held outside.
    /// - The borrowed data must not be mutated through another `&mut` while the
    ///   caller still holds the reference handed out by this `yield`, i.e. no
    ///   aliasing `&mut` may cross a `yield` boundary.
    #[inline]
    pub unsafe fn yield_unsafe(&mut self, v: T) -> Option<A> {
        self.yield_with(v);
        // atomic::compiler_fence(atomic::Ordering::Acquire);
        self.get_yield()
    }

    /// Drive an inner generator `g`, re-yielding every value it produces and
    /// forwarding the sent parameter into it. Returns the last sent parameter.
    ///
    /// # Safety
    ///
    /// In addition to the contract of [`yield_unsafe`](Self::yield_unsafe)
    /// (every value produced by `g` is re-yielded through this `Scope`, so the
    /// same borrow rules apply to them), the caller must guarantee:
    ///
    /// - `g` is an inner generator whose `A`/`T` match this `Scope`'s `A`/`T`,
    ///   so that values and parameters line up. The `Generator<A, T>` parameter
    ///   enforces that the element types match, but the caller remains
    ///   responsible for the value/lifetime semantics of what `g` yields.
    /// - `g` must not outlive, nor escape, this generator. It is driven to
    ///   completion and dropped inside this call, so its stack must stay alive
    ///   for the whole loop; do not move `g` out or share it across a `yield`
    ///   boundary.
    /// - As with `yield_unsafe`, any reference re-yielded from `g` must remain
    ///   valid for as long as the outer `send`/`resume` caller can observe it.
    pub unsafe fn yield_from_unsafe(&mut self, mut g: Generator<A, T>) -> Option<A> {
        let env = ContextStack::current();
        let context = env.top();
        let mut p = self.get_yield();
        while !g.is_done() {
            let r = g.raw_send(p)?;
            self.raw_yield(&env, context, r);
            p = self.get_yield();
        }
        drop(g); // explicitly consume g
        p
    }
}

impl<A, T> Scope<'_, 'static, A, T> {
    /// yield and get the send para
    // it's totally safe that we can refer to the function block
    // since we will come back later
    #[inline]
    pub fn yield_(&mut self, v: T) -> Option<A> {
        unsafe { self.yield_unsafe(v) }
    }

    /// `yield_from`
    /// the from generator must has the same type as itself
    pub fn yield_from(&mut self, g: Generator<A, T>) -> Option<A> {
        unsafe { self.yield_from_unsafe(g) }
    }
}
