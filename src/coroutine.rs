//! # generator
//!
//! coroutine implementation
//! coroutine is static life time generator
//! rust doesn't support get typeid for non-static types
//! and doesn't support specilization for life time
//! and doesn't support struct inheriant
//! so we create this static life time generator wrapper
//! this is ugly but we have to now
//! what we need is just override the drop implementation
//!

use std::any::Any;
use std::marker::PhantomData;

use generator::Generator;
use gen_impl::*;
use stack_size::{get_stack_size, set_stack_size};

/// Coroutine helper
pub struct Co<A> {
    dummy: PhantomData<A>,
}

impl <A: Any> Co<A> {
    /// create a new generator with default stack size
    pub fn new<T: Any, F>(f: F) -> Box<Generator<A, Output = T> + 'static>
        where F: FnOnce() -> T + Any
    {
        Self::new_opt(f, DEFAULT_STACK_SIZE - 1)
    }

    /// create a new generator with specified stack size
    pub fn new_opt<T: Any, F>(f: F, size: usize) -> Box<Generator<A, Output = T> + 'static>
        where F: FnOnce() -> T + Any
    {
        let g = Box::new(CoroutineImpl::<A, T, F>::new(f, size));
        g.init()
    }
}

/// CoroutineImpl
struct CoroutineImpl<A: Any, T: Any, F>
    where F: FnOnce() -> T + Any
{
    super_: GeneratorImpl<A, T, F>,
}

impl<A: Any, T: Any, F> CoroutineImpl<A, T, F>
    where F: FnOnce() -> T + Any
{
    /// create a new coroutine with specified stack size
    fn new(f: F, size: usize) -> Self {
        let mut size = size;
        let record = get_stack_size::<F>();
        if record != 0 {
            size = record;
        }
        CoroutineImpl { super_: GeneratorImpl::<A, T, F>::new(f, size) }
    }

    /// init the data within heap
    fn init(self: Box<Self>) -> Box<Self> {
        unsafe {
            let ptr = Box::into_raw(self);
            let g = &mut (*ptr).super_ as *mut GeneratorImpl<A, T, F>;
            let b = Box::from_raw(g);
            let b = b.init();
            Box::into_raw(b);
            Box::from_raw(ptr)
        }
    }
}

impl<A: Any, T: Any, F> Drop for CoroutineImpl<A, T, F>
    where F: FnOnce() -> T + Any
{
    fn drop(&mut self) {
        let (total_stack, used_stack) = self.stack_usage();
        // only record when the stack size is odd
        if (total_stack & 1) != 0 {
            set_stack_size::<F>(used_stack);
        }
    }
}

impl<A: Any, T: Any, F> Generator<A> for CoroutineImpl<A, T, F>
    where F: FnOnce() -> T + Any
{
    type Output = T;

    fn raw_send(&mut self, para: Option<A>) -> Option<T> {
        self.super_.raw_send(para)
    }

    fn cancel(&mut self) {
        self.super_.cancel()
    }

    fn is_done(&self) -> bool {
        self.super_.is_done()
    }

    fn stack_usage(&self) -> (usize, usize) {
        self.super_.stack_usage()
    }
}
