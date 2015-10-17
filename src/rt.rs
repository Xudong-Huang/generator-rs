//! # generator run time support
//!
//! generator run time context management
//!

use std::cell::UnsafeCell;
use Context;

/// each thread has it's own generator context stack
thread_local!(static CONTEXT_STACK: UnsafeCell<Box<ContextStack>>
                                  = UnsafeCell::new(ContextStack::new()));

thread_local!(static ROOT_CONTEXT: UnsafeCell<Context>
                                 = UnsafeCell::new(Context::empty()));

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

