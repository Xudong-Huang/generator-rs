//! # generator
//!
//! Rust generator implementation
//!


use std::mem;
use std::thread;
use std::any::Any;
use std::boxed::FnBox;

use yield_::yield_now;
use generator::Generator;
use rt::{Error, Context, ContextStack};
use reg_context::Context as RegContext;
use stack_size::{get_stack_size, set_stack_size};

/// GeneratorImpl
pub struct GeneratorImpl<A: Any, T: Any, F>
    where F: FnOnce() -> T + Any
{
    context: Context,
    // save the input
    para: Option<A>,
    // save the output
    ret: Option<T>,
    // boxed functor
    f: Option<F>,
}

impl<'a, A: Any, T: Any, F> GeneratorImpl<A, T, F>
    where F: FnOnce() -> T + 'a + Any
{
    /// create a new generator with default stack size
    pub fn new_opt(f: F, size: usize) -> Box<Generator<A, Output = T> + 'a> {
        let f = Some(f);

        let mut size = size;
        if size == 1024 {
            size = get_stack_size(&f);
            if size == 0{
                size = 1024;
            };
        }

        let mut g = Box::new(GeneratorImpl {
            para: None,
            ret: None,
            f: f,
            context: Context::new(size),
        });

        error!("stack size is {}", size);

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
}

impl<A: Any, T: Any, F> Drop for GeneratorImpl<A, T, F>
    where F: FnOnce() -> T + Any
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

        let (total_stack, used_stack)  = self.stack_usage();
        if used_stack < total_stack {
            // here we should record the stack in the class
            // next time will just use
            error!("total stack size is: {} words", total_stack);
            error!("used stack size is: {} words", used_stack);
            set_stack_size(&self.f, used_stack + 100);
        } else {
            error!("stack overflow detected!");
            panic!(Error::StackErr);
        }
    }
}

impl<A: Any, T: Any, F> Generator<A> for GeneratorImpl<A, T, F>
    where F: FnOnce() -> T + Any
{
    type Output = T;

    fn raw_send(&mut self, para: Option<A>) -> Option<T> {
        if self.is_started() && self.context._ref != 0 {
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

    fn is_done(&self) -> bool {
        self.is_started() && self.context._ref != 0
    }

    fn stack_usage(&self) -> (usize, usize) {
        (self.context.stack.size(),
         self.context.stack.get_used_size())
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
        if let Err(cause) = thread::catch_panic(clo) {
            if cause.downcast_ref::<Error>().is_some() {
                match cause.downcast_ref::<Error>().unwrap() {
                    &Error::Cancel => {}
                    err => {
                        let ctx = ContextStack::current().top();
                        ctx.err = Some(*err);
                    }
                }
            } else {
                error!("Panicked inside: {:?}", cause.downcast::<&str>());
            }
        }
        // drop the clo here
    }

    yield_now();

    unreachable!("Should never comeback");
}
