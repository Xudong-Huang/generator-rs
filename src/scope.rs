//! # yeild
//!
//! generator yield implmentation
//!

// use generator::Generator;
use gen_impl::GeneratorImpl;
use rt::{Error, Context, ContextStack};
use yield_::raw_yield_now;

/// passed in scope tpye
/// it not use the context to pass data, but keep it's own data ref
/// this struct provide both compile type info and runtime data
pub struct Scope<A, T> {
    para: *mut Option<A>,
    ret: *mut Option<T>,
}

impl<A, T> Scope<A, T> {
    /// create a new scope object
    pub fn new(para: *mut Option<A>, ret: *mut Option<T>) -> Self {
        Scope {
            para: para,
            ret: ret,
        }
    }

    /// set current generator return value
    #[inline]
    fn set_ret(&mut self, v: T) {
        let ret = unsafe { &mut *self.ret };
        *ret = Some(v);
    }

    /// get current generator send para
    #[inline]
    fn get_para(&mut self) -> Option<A> {
        let para = unsafe { &mut *self.para };
        para.take()
    }

    /// raw yiled without catch passed in para
    #[inline]
    fn raw_yield(&mut self, env: &mut ContextStack, context: &mut Context, v: T) {
        // check the context
        if !context.is_generator() {
            info!("yield from none generator context");
            // do nothing, just return
            return;
            // panic!(Error::ContextErr);
        }

        self.set_ret(v);
        context._ref -= 1;
        raw_yield_now(env, context);

        // here we just panic to exit the func
        if context._ref != 1 {
            panic!(Error::Cancel);
        }
    }

    /// yiled and get the send para
    // it's totally safe that we can refer to the function block
    // since we will come back later
    #[inline]
    pub fn yield_(&mut self, v: T) -> Option<A> {
        let env = ContextStack::current();
        let context = env.top();
        self.raw_yield(env, context, v);
        self.get_para()
    }


    /// `yiled_from`
    /// the from generator must has the same type as itself
    pub fn yield_from(&mut self, mut g: Box<GeneratorImpl<A, T>>) -> Option<A> {
        let env = ContextStack::current();
        let context = env.top();
        let mut p = self.get_para();
        while !g.is_done() {
            let r = g.raw_send(p).unwrap();
            self.raw_yield(env, context, r);
            p = self.get_para();
        }
        p
    }
}
