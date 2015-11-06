//! # generator
//!
//! coroutine implementation
//! coroutine is static life time generator
//! rust doesn't support get typeid for non-static types
//! and doesn't support specilization for life time
//! so we create this static life time generator wrapper
//! this is ugly but we have to now
//!

use std::mem;
use std::thread;
use std::any::Any;
use std::boxed::FnBox;
use std::marker::PhantomData;

use yield_::yield_now;
use generator::Generator;
use rt::{Error, Context, ContextStack};
use reg_context::Context as RegContext;
// use stack_size::{get_stack_size, set_stack_size};


/// Coroutine helper
pub struct Co<A> {
    dummy: PhantomData<A>,
}

impl <A: Any> Co<A> {
    /// create a new generator with default stack size
    pub fn new<T: Any, F>(f: F) -> Box<Generator<A, Output = T> + 'static>
        where F: FnOnce() -> T + 'static
    {
        Self::new_opt(f, super::stack::DEFAULT_STACK_SIZE)
    }

    /// create a new generator with specified stack size
    pub fn new_opt<T: Any, F>(f: F, size: usize) -> Box<Generator<A, Output = T> + 'static>
        where F: FnOnce() -> T + 'static
    {
        let mut g = Box::new(CoroutineImpl::<A, T, F>::new_opt(f, size));

        g.context.para = &mut g.para as &mut Any;
        g.context.ret = &mut g.ret as &mut Any;

        unsafe {
            let ptr = Box::into_raw(g);

            let start: Box<FnBox()> = Box::new(move || {
                // g.ret = Some((g.f.take().unwrap())());
                let f = (*ptr).f.take().unwrap();
                (*ptr).ret = Some(f());
            });

            let stk = &mut (*ptr).context.stack;
            let reg = &mut (*ptr).context.regs;
            reg.init_with(gen_init,
                          ptr as usize,
                          Box::into_raw(Box::new(start)) as *mut usize,
                          stk.end());
            Box::from_raw(ptr)
        }
    }
}

/// GeneratorImpl
struct GeneratorImpl<A: Any, T: Any, F>
    where F: FnOnce() -> T
{
    inner: GeneratorImpl<A, T, F>,
}

impl<A: Any, T: Any, F> CoroutineImpl<A, T, F>
    where F: FnOnce() -> T + 'static
{
    /// create a new coroutine with specified stack size
    fn new_opt(f: F, size: usize) -> Self {
        CoroutineImpl { inner: GeneratorImpl::new_opt(f, size) }
    }
}

impl<A: Any, T: Any, F> Drop for CoroutineImpl<A, T, F>
    where F: FnOnce() -> T
{
    fn drop(&mut self) {
        let (total_stack, used_stack) = self.stack_usage();
        if used_stack < total_stack {
        }
    }
}

impl<A: Any, T: Any, F> Generator<A> for CoroutineImpl<A, T, F>
    where F: FnOnce() -> T
{
    type Output = T;

    fn raw_send(&mut self, para: Option<A>) -> Option<T> {
        self.inner.raw_send(para)
    }

    fn cancel(&mut self) {
        self.inner.cancel()
    }

    fn is_done(&self) -> bool {
        self.inner.is_done()
    }

    fn stack_usage(&self) -> (usize, usize) {
        self.inner.stack_usage()
    }
}
