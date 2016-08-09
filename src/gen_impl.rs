//! # generator
//!
//! Rust generator implementation
//!

use std::mem;
use std::panic;
use std::thread;
use std::any::Any;
use std::boxed::FnBox;
use std::marker::PhantomData;

use scope::Scope;
use yield_::yield_now;
use generator::Generator;
use rt::{Error, Context, ContextStack};
use reg_context::Context as RegContext;

// default stack size, in usize
// windows has a minimal size as 0x4a8!!!!
pub const DEFAULT_STACK_SIZE: usize = 0x800;

/// Generator helper
pub struct Gn<A> {
    dummy: PhantomData<A>,
}

impl<A> Gn<A> {
    /// create a scoped generator
    pub fn new_scoped<'a, T, F>(f: F) -> Box<Generator<A, Output = T> + 'a>
        where F: FnOnce(Scope<A, T>) -> T + 'a,
              T: 'a,
              A: 'a
    {
        let mut g = GeneratorImpl::<A, T, _>::new(DEFAULT_STACK_SIZE);
        let scope = g.get_scope();
        g.init(move || f(scope));
        g
    }
}

impl<A: Any> Gn<A> {
    /// create a new generator with default stack size
    pub fn new<'a, T: Any, F>(f: F) -> Box<Generator<A, Output = T> + 'a>
        where F: FnOnce() -> T + 'a
    {
        Self::new_opt(f, DEFAULT_STACK_SIZE)
    }

    /// create a new generator with specified stack size
    pub fn new_opt<'a, T: Any, F>(f: F, size: usize) -> Box<Generator<A, Output = T> + 'a>
        where F: FnOnce() -> T + 'a
    {
        let mut g = GeneratorImpl::<A, T, _>::new(size);
        g.init_context();
        g.init(f);
        g
    }
}

/// `GeneratorImpl`
pub struct GeneratorImpl<A, T, F>
    where F: FnOnce() -> T
{
    // run time context
    context: Context,
    // save the input
    para: Option<A>,
    // save the output
    ret: Option<T>,
    // boxed functor
    f: Option<F>,
}

impl<A: Any, T: Any, F> GeneratorImpl<A, T, F>
    where F: FnOnce() -> T
{
    /// create a new generator with default stack size
    pub fn init_context(&mut self) {
        self.context.para = &mut self.para as &mut Any;
        self.context.ret = &mut self.ret as &mut Any;
    }
}

impl<A, T, F> GeneratorImpl<A, T, F>
    where F: FnOnce() -> T
{
    /// create a new generator with specified stack size
    pub fn new(size: usize) -> Box<Self> {
        Box::new(GeneratorImpl {
            para: None,
            ret: None,
            f: None,
            context: Context::new(size),
        })
    }

    /// get the scope object
    pub fn get_scope(&mut self) -> Scope<A, T> {
        Scope::new(&mut self.para, &mut self.ret)
    }

    /// init a heap based generator
    // it's can be used to re-init a 'done' generator before it's get dropped
    pub fn init(&mut self, f: F) {
        self.f = Some(f);
        unsafe {
            let ptr = self as *mut Self;

            let start: Box<FnBox()> = Box::new(move || {
                let f = (*ptr).f.take().unwrap();
                (*ptr).ret = Some(f());
            });

            let stk = &(*ptr).context.stack;
            let reg = &mut (*ptr).context.regs;
            reg.init_with(gen_init,
                          ptr as usize,
                          Box::into_raw(Box::new(start)) as *mut usize,
                          stk);
        }
    }

    /// resume the generator
    #[inline]
    fn resume_gen(&mut self) {
        let env = ContextStack::current();
        let cur = &mut env.top().regs;
        let ctx = &mut self.context as *mut Context;
        let to = unsafe { &mut (*ctx).regs };
        // save current generator context on stack
        env.push(ctx);
        // switch context
        RegContext::swap(cur, to);

        // check the panic status
        // this would propagate the panic until root context
        let err = self.context.err;
        if err.is_some() {
            panic!(err.unwrap());
        }
    }

    #[inline]
    fn is_started(&self) -> bool {
        // when the f is consumed we think it's running
        self.f.is_none()
    }

    #[inline]
    fn raw_send(&mut self, para: Option<A>) -> Option<T> {
        if self.is_done() {
            return None;
        }

        // this is the passed in value of the send primitive
        // the yield part would read out this value in the next round
        // use the replace is would drop the old value
        mem::replace(&mut self.para, para);

        // every time we call the function, increase the ref count
        // yiled will decrease it and return will not
        self.context._ref += 1;
        self.resume_gen();

        self.ret.take()
    }

    fn cancel(&mut self) {
        // consume the fun if it's not started
        if !self.is_started() {
            self.f.take();
            self.context._ref = 1;
        } else {
            // tell the func to panic
            // so that we can stop the inner func
            self.context._ref = 2;
            self.resume_gen();
        }
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.is_started() && self.context._ref != 0
    }

    fn stack_usage(&self) -> (usize, usize) {
        (self.context.stack.size(), self.context.stack.get_used_size())
    }
}

impl<A, T, F> Drop for GeneratorImpl<A, T, F>
    where F: FnOnce() -> T
{
    fn drop(&mut self) {
        // when the thread is already panic, do nothing
        if thread::panicking() {
            return;
        }

        let mut i = 0;
        while !self.is_done() {
            if i > 2 {
                self.cancel();
                break;
            }
            self.raw_send(None);
            i += 1;
        }

        let (total_stack, used_stack) = self.stack_usage();
        if used_stack < total_stack {
            // here we should record the stack in the class
            // next time will just use
            // set_stack_size::<F>(used_stack);
        } else {
            error!("stack overflow detected!");
            panic!(Error::StackErr);
        }
    }
}

impl<A, T, F> Generator<A> for GeneratorImpl<A, T, F>
    where F: FnOnce() -> T
{
    type Output = T;

    #[inline]
    fn raw_send(&mut self, para: Option<A>) -> Option<T> {
        self.raw_send(para)
    }

    #[inline]
    fn cancel(&mut self) {
        self.cancel();
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.is_done()
    }

    #[inline]
    fn stack_usage(&self) -> (usize, usize) {
        self.stack_usage()
    }
}

fn gen_init(_: usize, f: *mut usize) -> ! {
    {
        let f = f as usize;
        let clo = move || {
            let func: Box<Box<FnBox()>> = unsafe { Box::from_raw(f as *mut Box<FnBox()>) };
            func();
        };

        // we can't panic inside the generator context
        // need to propagate the panic to the main thread
        // It is currently undefined behavior to unwind from Rust code into foreign code
        if let Err(cause) = panic::catch_unwind(clo) {
            if cause.downcast_ref::<Error>().is_some() {
                match cause.downcast_ref::<Error>().unwrap() {
                    &Error::Cancel => {}
                    err => {
                        let ctx = ContextStack::current().top();
                        ctx.err = Some(*err);
                    }
                }
            } else {
                // we hope all other panic could covert to a string
                // here we forget the panic to avoid shutdown the whole thread
                let e = cause.downcast::<&str>().unwrap_or_else(|_| Box::new("unkown panic"));
                error!("Panicked inside: {:?}", e);
            }
        }
        // drop the clo here
    }

    yield_now();

    unreachable!("Should never comeback");
}
