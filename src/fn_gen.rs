//! # generator
//!
//! Rust generator implementation
//!


use std::mem;
use std::thread;
use std::any::Any;
use std::boxed::FnBox;

use Generator;
use yield_::yield_now;
use rt::{Error, Context, ContextStack};
use reg_context::Context as RegContext;

// default stack size is 1k * sizeof(usize)
const DEFAULT_STACK_SIZE: usize = 1024;

/// FnGenerator
pub struct FnGenerator<'a, A: Any, T: Any> {
    context: Context,
    // save the input
    para: Option<A>,
    // save the output
    ret: Option<T>,
    // boxed functor
    f: Option<Box<FnBox() -> T + 'a>>,
}

impl<'a, A: Any, T: Any> FnGenerator<'a, A, T> {
    /// create a new generator
    pub fn new<F>(f: F) -> Box<Generator<A, Output = T> + 'a>
        where F: FnOnce() -> T + 'a
    {
        let mut g = Box::new(FnGenerator {
            para: None,
            ret: None,
            f: Some(Box::new(f)),
            context: Context::new(DEFAULT_STACK_SIZE),
        });

        g.context.para = &mut g.para as &mut Any;
        g.context.ret = &mut g.ret as &mut Any;

        unsafe {
            let ptr = Box::into_raw(g);

            let start: Box<FnBox()> = Box::new(move || {
                // here seems that rust test --release has bug!!
                // comment out the following would crash
                // error!("{:?}", ptr);
                // let ref mut g = *ptr;
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

impl<'a, A: Any, T: Any> Drop for FnGenerator<'a, A, T> {
    fn drop(&mut self) {
        let mut i = 0;
        while !self.is_done() {
            if i > 2 {
                self.cancel();
                break;
            }
            self.raw_send(None);
            i += 1;
        }

        let used_stack = self.context.stack.get_used_size();
        if used_stack < self.context.stack.size() {
            // here we should record the stack in the class
            // next time will just use
            info!("used stack size is: {} words", used_stack)
        } else {
            error!("stack overflow detected!");
            panic!(Error::StackErr);
        }
    }
}

impl<'a, A: Any, T: Any> Generator<A> for FnGenerator<'a, A, T> {
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
