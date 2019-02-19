//! # generator run time support
//!
//! generator run time context management
//!
use std::any::Any;
use std::mem;
use std::ptr;

use reg_context::RegContext;
use stack::Stack;

thread_local!(
    /// each thread has it's own generator context stack
    static ROOT_CONTEXT: Box<Context> = {
        let mut root = Box::new(Context::empty());
        let p = &mut *root as *mut _;
        root.parent = p; // init top to current
        root
    }
);

// fast access pointer, this is will be init only once
// when ROOT_CONTEXT get inialized. but in debug mode it
// will be zero in generator context since the stack changed
// to a different place, be careful about that.
#[cfg(nightly)]
#[thread_local]
static mut ROOT_CONTEXT_P: *mut Context = ptr::null_mut();

/// yield panic error types
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Error {
    /// Cancel panic
    Cancel,
    /// Type mismatch panic
    TypeErr,
    /// Stack overflow panic
    StackErr,
    /// Wrong Context panic
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
    // create for root empty context
    fn empty() -> Self {
        Context {
            regs: RegContext::empty(),
            stack: Stack::empty(),
            para: unsafe { mem::uninitialized() },
            ret: unsafe { mem::uninitialized() },
            _ref: 1, // none zero means it's not running
            err: None,
            child: ptr::null_mut(),
            parent: ptr::null_mut(),
            local_data: ptr::null_mut(),
        }
    }

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
        self.parent != self as *const _ as *mut _
    }

    /// get current generator send para
    #[inline]
    pub fn get_para<A>(&self) -> Option<A>
    where
        A: Any,
    {
        let para = unsafe { &mut *self.para };
        match para.downcast_mut::<Option<A>>() {
            Some(v) => v.take(),
            None => type_error::<A>("get yield type mismatch error detected"),
        }
    }

    /// set current generator send para
    #[inline]
    pub fn set_para<A>(&self, data: A)
    where
        A: Any,
    {
        let para = unsafe { &mut *self.para };
        match para.downcast_mut::<Option<A>>() {
            Some(v) => *v = Some(data),
            None => type_error::<A>("set yield type mismatch error detected"),
        }
    }

    /// set current generator return value
    #[inline]
    pub fn set_ret<T>(&mut self, v: T)
    where
        T: Any,
    {
        let ret = unsafe { &mut *self.ret };
        match ret.downcast_mut::<Option<T>>() {
            Some(r) => *r = Some(v),
            None => type_error::<T>("yield type mismatch error detected"),
        }
    }
}

/// Coroutine managing environment
pub struct ContextStack {
    root: *mut Context,
}

#[cfg(nightly)]
#[cold]
#[inline(never)]
unsafe fn init_root_p() {
    ROOT_CONTEXT_P = ROOT_CONTEXT.with(|r| &**r as *const _ as *mut Context);
}

impl ContextStack {
    #[cfg(nightly)]
    #[inline]
    pub fn current() -> ContextStack {
        unsafe {
            if ROOT_CONTEXT_P.is_null() {
                init_root_p();
            }
            ContextStack {
                root: ROOT_CONTEXT_P,
            }
        }
    }

    #[cfg(not(nightly))]
    #[inline]
    pub fn current() -> ContextStack {
        let root = ROOT_CONTEXT.with(|r| &**r as *const _ as *mut Context);
        ContextStack { root }
    }

    /// get the top context
    #[inline]
    pub fn top(&self) -> &'static mut Context {
        let root = unsafe { &mut *self.root };
        unsafe { &mut *root.parent }
    }

    /// get the coroutine context
    #[inline]
    pub fn co_ctx(&self) -> Option<&'static mut Context> {
        let root = unsafe { &mut *self.root };

        // search from top
        let mut ctx = unsafe { &mut *root.parent };
        while ctx as *const _ != root as *const _ {
            if !ctx.local_data.is_null() {
                return Some(ctx);
            }
            ctx = unsafe { &mut *ctx.parent };
        }
        // not find any coroutine
        None
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

#[inline]
fn type_error<A>(msg: &str) -> ! {
    #[cfg(nightly)]
    {
        use std::intrinsics::type_name;
        let t = unsafe { type_name::<A>() };
        error!("{}, expected type: {}", msg, t);
    }

    #[cfg(not(nightly))]
    {
        error!("{}", msg);
    }
    panic!(Error::TypeErr)
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

    // search from top
    let mut ctx = unsafe { &mut *root.parent };
    while ctx as *const _ != root as *const _ {
        if !ctx.local_data.is_null() {
            return ctx.local_data;
        }
        ctx = unsafe { &mut *ctx.parent };
    }

    ptr::null_mut()
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
