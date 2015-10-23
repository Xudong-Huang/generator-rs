//! # generator run time support
//!
//! generator run time context management
//!

use std::mem;
use std::any::Any;
use std::cell::UnsafeCell;
use std::rt::util::min_stack;
use context::Stack;
use context::Context as RegContext;

/// each thread has it's own generator context stack
thread_local!(static CONTEXT_STACK: UnsafeCell<Box<ContextStack>>
                                  = UnsafeCell::new(ContextStack::new()));

thread_local!(static ROOT_CONTEXT: UnsafeCell<Context>
                                 = UnsafeCell::new(Context::empty()));

/// generator context
pub struct Context {
    /// generator regs context
    pub regs: RegContext,
    /// generator execution stack
    pub stack: Stack,
    /// passed in para for send
    pub para: *mut Any,
    /// this is just a buffer for the return value
    pub ret:  *mut Any,
    /// track generator ref, yield will -1, send will +1
    pub _ref: u32,
    /// priave flag that control the execution flow
    pub _flag: bool,
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
            _ref: 100, // any number that is bigger than 2
            _flag: false,
        }
    }

    /// judge it's generator context
    pub fn is_generator(&self) -> bool {
        // TODO use stack empty to check
        self._ref < 2
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

/// Coroutine managing environment
pub struct ContextStack {
    stack: Vec<*mut Context>
}

impl ContextStack {
    fn new() -> Box<ContextStack> {
        let mut r = Box::new(ContextStack {stack: Vec::new()});
        r.push(ROOT_CONTEXT.with(|env| env.get()));
        r
    }

    /// get current thread's generator context stack
    #[inline]
    pub fn current() -> &'static mut ContextStack {
        CONTEXT_STACK.with(|env| unsafe {&mut *env.get()})
    }

    /// push generator context
    #[inline]
    pub fn push(&mut self, context: *mut Context) {
        self.stack.push(context);
    }

    /// pop generator context
    #[inline]
    pub fn pop(&mut self) -> Option<*mut Context> {
        self.stack.pop()
    }

    /// get current generator context
    #[inline]
    pub fn top(&self) -> &'static mut Context {
        unsafe {&mut **self.stack.last().unwrap()}
    }
}

#[cfg(test)]
mod test {
    use super::ContextStack;

    #[test]
    fn test_is_context() {
        // this is the root context
        let ctx = ContextStack::current().top();
        assert!(!ctx.is_generator());
    }
}

