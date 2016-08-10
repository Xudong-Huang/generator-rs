//! # generator run time support
//!
//! generator run time context management
//!

use std::mem;
use std::any::Any;
use std::cell::UnsafeCell;
use std::intrinsics::type_name;

use stack::Stack;
use reg_context::Context as RegContext;

/// each thread has it's own generator context stack
thread_local!(static CONTEXT_STACK: UnsafeCell<ContextStack> = ContextStack::new());
thread_local!(static ROOT_CONTEXT: Context = Context::new(0));

/// yield error types
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Error {
    Cancel,
    TypeErr,
    StackErr,
    ContextErr,
}

/// generator context
pub struct Context {
    /// generator regs context
    pub regs: RegContext,
    /// generator execution stack
    pub stack: Stack,
    /// passed in para for send
    pub para: *mut Any,
    /// this is just a buffer for the return value
    pub ret: *mut Any,
    /// track generator ref, yield will -1, send will +1
    pub _ref: u32,
    /// propagate panic
    pub err: Option<Error>,
}

impl Context {
    /// return a default generator context
    pub fn new(size: usize) -> Context {
        Context {
            regs: RegContext::empty(),
            stack: Stack::new(size),
            para: unsafe { mem::uninitialized() },
            ret: unsafe { mem::uninitialized() },
            _ref: 1, // none zero means it's not running
            err: None,
        }
    }

    /// judge it's generator context
    #[inline]
    pub fn is_generator(&self) -> bool {
        self.stack.size() > 0
    }

    /// get current generator send para
    #[inline]
    pub fn get_para<A>(&self) -> Option<A>
        where A: Any
    {
        let para = unsafe { &mut *self.para };
        let val = para.downcast_mut::<Option<A>>();
        if val.is_some() {
            val.unwrap().take()
        } else {
            let t = unsafe { type_name::<A>() };
            error!("get yield type error detected, expected type: {}", t);
            panic!(Error::TypeErr);
        }
    }

    /// set current generator return value
    #[inline]
    pub fn set_ret<T>(&mut self, v: T)
        where T: Any
    {
        let ret = unsafe { &mut *self.ret };
        let val = ret.downcast_mut::<Option<T>>();
        if val.is_some() {
            mem::replace(val.unwrap(), Some(v));
        } else {
            let t = unsafe { type_name::<T>() };
            error!("yield type error detected, expected type: {}", t);
            panic!(Error::TypeErr);
        }
    }
}

/// Coroutine managing environment
pub struct ContextStack {
    stack: Vec<*mut Context>,
}

impl ContextStack {
    fn new() -> UnsafeCell<ContextStack> {
        let env = UnsafeCell::new(ContextStack { stack: Vec::with_capacity(16) });
        let stack = unsafe { &mut *env.get() };
        stack.push(ROOT_CONTEXT.with(|env| unsafe { &mut *(env as *const _ as *mut _) }));
        // here we have no changce to drop the context stack
        env
    }

    /// get current thread's generator context stack
    #[inline]
    pub fn current() -> &'static mut ContextStack {
        CONTEXT_STACK.with(|env| unsafe { &mut *env.get() })
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
        unsafe { &mut **self.stack.last().unwrap() }
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
