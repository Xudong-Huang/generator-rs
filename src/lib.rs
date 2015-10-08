//! # generator
//!
//! Rust generator library
//!

#![feature(fnbox)]
#![feature(rustc_private)]
#![feature(rt)]
#![feature(box_raw)]
#![cfg_attr(test, deny(warnings))]
#![deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate libc;
extern crate context;

pub mod env;
pub use env::ContextStack;

use context::Stack;
use context::Context as RegContext;
use std::rt::unwind::try;
use std::rt::util::min_stack;
use std::boxed::FnBox;
use std::any::Any;
use std::mem;
use std::ptr;

/// generator context
pub struct Context {
    /// generator regs context
    regs: RegContext,
    /// generator execution stack
    stack: Stack,
    /// passed in para for send
    para: *mut Any,
    /// this is just a buffer for the return value
    ret:  *mut Any,
    /// track generator ref, yield will -1, send will +1
    _ref: u32,
    /// priave flag that control the execution flow
    _flag: bool,
}

impl Context {
    /// return a default generator context
    pub fn new() -> Context {
        Context {
            regs: RegContext::empty(),
            stack: Stack::new(min_stack()),
            para: unsafe { mem::transmute(&0 as &Any) },
            ret: unsafe { mem::transmute(&0 as &Any) },
            _ref: 0,
            _flag: false,
        }
    }

    /// empty context used for the normal thread
    pub fn empty() -> Context {
        Context {
            regs: RegContext::empty(),
            stack: unsafe { Stack::dummy_stack() },
            para: unsafe { mem::transmute(&0 as &Any) },
            ret: unsafe { mem::transmute(&0 as &Any) },
            _ref: 0,
            _flag: false,
        }
    }

    /// save current generator context
    #[inline(always)]
    pub fn save(&mut self) {
        self._ref -= 1;
        self._flag = true;
        RegContext::save(&mut self.regs);
    }

    /// generator involke flag, save it along with context
    #[inline]
    pub fn get_flag(&mut self)-> &'static mut bool {
        unsafe {mem::transmute(&mut self._flag)}
    }

    /// get current generator send para
    #[inline]
    pub fn get_para<T>(&self) -> Option<T> where T: Any {
        let para = unsafe { &mut *self.para };
        let val = para.downcast_mut::<Option<T>>().unwrap();
        val.take()
    }

    /// set current generator return value
    #[inline]
    pub fn set_ret<T>(&mut self, v: T) where T: Any {
        let ret = unsafe { &mut *self.ret };
        // add type check and panic with message
        let val = ret.downcast_mut::<Option<T>>().unwrap();
        mem::replace(val, Some(v));
    }
}

/// generator trait
pub trait Generator<A> {
    /// Output type
    type Output;

    /// send interface
    fn send(&mut self, para: A) -> Self::Output;

    /// is finished
    fn is_done(&self) -> bool;
}

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


impl<A, T> FnGenerator<A, T> {
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

/// switch back to parent context
pub fn yield_now() {
    let env = ContextStack::current();
    let ctx = env.top();
    let sp = ctx.stack.start();
    // judge if this is root context
    if sp != ptr::null() {
        env.pop();
        let ctx = env.top();
        RegContext::load(&ctx.regs);
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

/// create generator
pub fn make_gen<A: Any, T: Any>(p: Option<A>, f: Box<FnBox()->T>) -> Box<FnGenerator<A, T>> {
    let mut g = Box::new(FnGenerator {
       para: p, ret: None, f: Some(f),
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


#[macro_export]
macro_rules! generator {
    // `(func, <type>)`
    // func: the expression for unsafe async function which contains yiled
    // para: default send para type to the generator
    ($func:expr, <$para:ty>) => (
        generator::make_gen::<$para, _>(None, Box::new(move|| {$func}))
    );

    // `(func, para)`
    // func: the expression for unsafe async function which contains yiled
    // para: default send para to the generator
    ($func:expr, $para:expr) => (
        generator::make_gen(Some($para), Box::new(move|| {$func}))
    );

    ($func:expr) => (generator!($func, ()));
}

/// yiled and get the send para
#[macro_export]
macro_rules! _yield {
    // `(para)`
    // val: the value that need to be yield
    // and got the send para from context
    ($val:expr) => ({
        _yield_!($val);
        generator::ContextStack::current().top().get_para().unwrap()
    });

    () => (_yield!(()));
}

/// yield without get the passed in para
#[macro_export]
macro_rules! _yield_ {
    // `(para)`
    // val: the value that need to be yield
    // and got the send para from context
    ($val:expr) => ({
        let context = generator::ContextStack::current().top();
        let _no_use = context.get_flag();
        context.save();
        if *_no_use {
            *_no_use = false;
            context.set_ret($val);
            // don't use the return instruction
            generator::yield_now();
            // context.load();
            return $val;
        }
    });

    () => (_yield_!(()));
}


