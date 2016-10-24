//! # generator run time support
//!
//! generator run time context management
//!
use std::mem;
use std::ptr;
use std::any::Any;
use std::intrinsics::type_name;

use stack::Stack;
use reg_context::Context as RegContext;

/// each thread has it's own generator context stack
thread_local!(static ROOT_CONTEXT: Box<Context> = {
    let mut root = Box::new(Context::new(0));
    let p = &mut *root as *mut _;
    root.parent = p; // init top to current
    root
});

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
    pub err: Option<Box<Any + Send>>,
    /// context local storage
    pub local_data: *mut u8,

    /// child context
    child: *mut Context,
    /// parent context
    pub parent: *mut Context,
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
            child: ptr::null_mut(),
            parent: ptr::null_mut(),
            local_data: ptr::null_mut(),
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
    root: *mut Context,
}

impl ContextStack {
    #[inline]
    pub fn current() -> ContextStack {
        let root = ROOT_CONTEXT.with(|r| &**r as *const _ as *mut Context);
        ContextStack { root: root }
    }

    /// get the top context
    #[inline]
    pub fn top(&self) -> &'static mut Context {
        let root = unsafe { &mut *self.root };
        unsafe { &mut *root.parent }
    }

    /// get the coroutine context
    #[inline]
    pub fn co_ctx(&self) -> &'static mut Context {
        let root = unsafe { &mut *self.root };
        // the root's child is used as the coroutine context pointer
        assert!(!root.child.is_null(), "there is no child of root!");
        unsafe { &mut *root.child }
    }

    /// push the context to the thread context list
    #[inline]
    pub fn push_context(&self, ctx: *mut Context) {
        let root = unsafe { &mut *self.root };
        let ctx = unsafe { &mut *ctx };
        let top = unsafe { &mut *root.parent };
        let new_top = ctx.parent;

        // link top and new ctx
        top.child = ctx;
        ctx.parent = top;

        // save the new top
        root.parent = new_top;
    }

    /// pop the context from the thread context list and return it's parent context
    #[inline]
    pub fn pop_context(&self, ctx: *mut Context) -> &'static mut Context {
        let root = unsafe { &mut *self.root };
        let ctx = unsafe { &mut *ctx };
        let parent = unsafe { &mut *ctx.parent };

        // save the old top in ctx's parent
        ctx.parent = root.parent;
        // unlink ctx and it's parent
        parent.child = ptr::null_mut();

        // save the new top
        root.parent = parent;

        parent
    }
}

/// check the current context if it's generator
#[inline]
pub fn is_generator() -> bool {
    let env = ContextStack::current();
    let root = unsafe { &mut *env.root };
    !root.child.is_null()
}

/// get the current context local data
/// only coroutine support local data
#[inline]
pub fn get_local_data() -> *mut u8 {
    let env = ContextStack::current();
    let root = unsafe { &mut *env.root };
    // the root's child is used as the coroutine context pointer
    if root.child.is_null() {
        return ptr::null_mut();
    }

    let child = unsafe { &*root.child };
    child.local_data
}


#[cfg(test)]
mod test {
    use super::is_generator;

    #[test]
    fn test_is_context() {
        // this is the root context
        assert!(!is_generator());
    }
}
