//! # generator
//!
//! Rust generator implementation
//!

use rt::Context;
use rt::ContextStack;
use {Generator, yield_now, Error};

use libc;
use std::thread;
use std::boxed::FnBox;
use std::any::Any;
use std::mem;
use context::Context as RegContext;

/// FnGenerator
pub struct FnGenerator<'a, A: Any, T: Any> {
    context: Context,
    // save the input
    para: Option<A>,
    // save the output
    ret: Option<T>,
    // boxed functor
    f: Option<Box<FnBox()->T + 'a>>
}

impl<'a, A: Any, T: Any> FnGenerator<'a, A, T> {
    /// create a new generator
    pub fn new<F>(f: F) -> Box<Generator<A, Output=T> + 'a>
        where F: FnOnce()->T + 'a
    {
        let mut g = Box::new(FnGenerator {
           para: None, ret: None, f: Some(Box::new(f)),
           context: Context::new()
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
            reg.init_with(gen_init, ptr as usize,
                          Box::into_raw(Box::new(start)) as *mut libc::c_void,
                          stk);
            Box::from_raw(ptr)
        }
    }

    fn resume_gen(&mut self) {
        let env = ContextStack::current();
        let cur = &mut env.top().regs;
        let ctx = &mut self.context as *mut Context;
        let to = unsafe {&mut (*ctx).regs};
        // save current generator context on stack
        env.push(ctx);
        // switch context
        RegContext::swap(cur, to);
    }

    #[inline]
    fn is_started(&self) -> bool {
        // when the f is consumed we think it's running
        self.f.is_none()
    }
}

impl<'a, A: Any, T: Any> Drop for FnGenerator<'a, A, T> {
    fn drop(&mut self){
        while !self.is_done() {
            self.raw_send(None);
        }
    }
}

impl<'a, A: Any, T: Any> Generator<A> for FnGenerator<'a, A, T> {
    type Output = T;

    fn raw_send(&mut self, para: Option<A>) -> Option<T> {
        if self.is_started() && self.context._ref != 0 {
            return None;
        }

        // every time we call the function, increase the ref count
        // yiled will decrease it and return will not
        self.context._ref += 1;
        self.resume_gen();

        // this is the passed in value of the send primitive
        // the yield part would read out this value in the next round
        // use the replace is would drop the old value
        mem::replace(&mut self.para, para);

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

#[allow(unused_variables)]
extern "C" fn gen_init(arg: usize, f: *mut libc::c_void) -> ! {
    let f = f as usize;
    let clo = move || {
        let func: Box<Box<FnBox()>> = unsafe {
            Box::from_raw(f as *mut Box<FnBox()>)
        };
        func();
    };

    if let Err(cause) = thread::catch_panic(clo){
        if cause.downcast_ref::<Error>().is_some() {
            match cause.downcast_ref::<Error>().unwrap() {
                & Error::Cancel => {},
                & Error::StackErr => { panic!("Stack overflow detected!"); },
                & Error::ContextErr => { panic!("Context error detected!"); },
            }
        } else {
            error!("Panicked inside: {:?}", cause.downcast::<&str>());
        }
    }

    yield_now();

    unreachable!("Should never comeback");
}

