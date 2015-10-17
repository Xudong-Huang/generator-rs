//! # generator
//!
//! Rust generator implementation
//!

use rt::Context;
use rt::ContextStack;
use super::{Generator, yield_now};

use libc;
use std::rt::unwind::try;
use std::boxed::FnBox;
use std::any::Any;
use std::mem;
use context::Context as RegContext;

/// FnGenerator
pub struct FnGenerator<A, T> {
    context: Context,
    // save the input
    para: Option<A>,
    // save the output
    ret: Option<T>,
    // boxed functor
    f: Option<Box<FnBox()->T>>
}


impl<A: Any, T: Any> FnGenerator<A, T> {
    /// create a new generator
    pub fn new<F>(f: F) -> Box<FnGenerator<A, T>>
        where F: FnOnce()->T + 'static
    {
        let mut g = Box::new(FnGenerator {
           para: None, ret: None, f: Some(Box::new(f)),
           context: Context::new()
        });

        g.context.para = &mut g.para as &mut Any;
        g.context.ret = &mut g.ret as &mut Any;

        unsafe {
            let ptr = Box::into_raw(g);

            //let start = Box::new(||{g.ret = Some((g.f.take().unwrap())())});
            let start: Box<FnBox()> = Box::new(move||{
                let mut g = Box::from_raw(ptr);
                g.ret = Some((g.f.take().unwrap())());
                // don't free the box here
                Box::into_raw(g);
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

    fn send_impl(&mut self, para: Option<A>) -> Option<T> {
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
}

impl<A: Any, T: Any> Generator<A> for FnGenerator<A, T> {
    type Output = T;

    fn send(&mut self, para: A) -> T {
        let ret = self.send_impl(Some(para));
        ret.unwrap()
    }

    fn is_done(&self) -> bool {
       self.is_started() && self.context._ref != 0
    }
}

impl<A: Any, T: Any> Iterator for FnGenerator<A, T> {
    type Item = T;
    // The 'Iterator' trait only requires the 'next' method to be defined. The
    // return type is 'Option<T>', 'None' is returned when the 'Iterator' is
    // over, otherwise the next value is returned wrapped in 'Some'
    fn next(&mut self) -> Option<T> {
        self.send_impl(None)
    }
}

#[allow(unused_variables)]
extern "C" fn gen_init(arg: usize, f: *mut libc::c_void) -> ! {
    {
        let func: Box<Box<FnBox()>> = unsafe {
            Box::from_raw(f as *mut Box<FnBox()>)
        };

        if let Err(cause) = unsafe { try(move|| func()) } {
            error!("Panicked inside: {:?}", cause.downcast::<&str>());
        }
    }

    yield_now();

    unreachable!("Should never comeback");
}

