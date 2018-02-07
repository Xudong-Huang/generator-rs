//! # yeild
//!
//! generator yield implmentation
//!

use std::marker::PhantomData;

use no_drop::NoDrop;
use yield_::raw_yield_now;
use gen_impl::GeneratorImpl;
use rt::{Context, ContextStack};

/// passed in scope tpye
/// it not use the context to pass data, but keep it's own data ref
/// this struct provide both compile type info and runtime data
pub struct Scope<A, T> {
    para: *mut Option<A>,
    phantom: PhantomData<T>,
}

impl<A, T> Scope<A, T> {
    /// create a new scope object
    pub fn new(para: *mut Option<A>) -> Self {
        Scope {
            para: para,
            phantom: PhantomData,
        }
    }

    /// raw yiled without catch passed in para
    #[inline]
    fn raw_yield(&mut self, env: &ContextStack, context: &mut Context, v: T) {
        // check the context
        if !context.is_generator() {
            #[cold]
            panic!("yield from none generator context");
        }

        let para = NoDrop::new(v).encode_usize();
        // here we just panic to exit the func
        raw_yield_now(env, context, &para as *const _ as usize)
    }

    /// yiled something without catch passed in para
    #[inline]
    pub fn yield_with(&mut self, v: T) {
        let env = ContextStack::current();
        let context = env.top();
        self.raw_yield(&env, context, v);
    }

    /// get current generator send para
    #[inline]
    pub fn get_yield(&mut self) -> Option<A> {
        let para = unsafe { &mut *self.para };
        para.take()
    }

    /// yiled and get the send para
    // it's totally safe that we can refer to the function block
    // since we will come back later
    #[inline]
    pub fn yield_(&mut self, v: T) -> Option<A> {
        self.yield_with(v);
        self.get_yield()
    }

    /// `yiled_from`
    /// the from generator must has the same type as itself
    pub fn yield_from(&mut self, mut g: Box<GeneratorImpl<A, T>>) -> Option<A> {
        let env = ContextStack::current();
        let context = env.top();
        let mut p = self.get_yield();
        while !g.is_done() {
            match g.raw_send(p) {
                None => return None,
                Some(r) => self.raw_yield(&env, context, r),
            }
            p = self.get_yield();
        }
        p
    }
}
