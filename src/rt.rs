//! # generator run time support
//!
//! generator run time context management
//!
use std::mem;
use std::ptr;
use std::any::{Any, TypeId};

use stack::Stack;
use reg_context::RegContext;
use detail::{swap, swap_link};

/// each thread has it's own generator context stack
thread_local!(static ROOT_CONTEXT: Box<Context> = {
    let mut root = Box::new(Context::root());
    let p = &mut *root as *mut _;
    root.parent = p; // init top to current
    root
});

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
    /// type id for yield out value
    pub ret_type: TypeId,
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
    // create for root context
    fn root() -> Self {
        Context {
            regs: RegContext::root(),
            stack: Stack::empty(),
            para: unsafe { mem::uninitialized() },
            ret_type: TypeId::of::<()>(),
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
            ret_type: TypeId::of::<()>(),
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

    /// Switch execution contexts to parent stack
    ///
    /// Suspend the current execution context and resume another by
    /// saving the registers values of the executing thread to a Context
    /// then loading the registers from a previously saved Context.
    /// after the peer call the swap again, this function would return
    /// the passed in arg would be catch by the peer swap and the return
    /// value is the peer swap arg
    ///
    /// usually we use NoDop and decode_usize/encode_usize to convert data
    /// between different stacks
    #[inline]
    pub fn swap_yield(&mut self, parent: &mut Context, arg: usize) -> usize {
        // we have finished the context stack pop
        // self is current generator context
        parent.regs.restore_context();
        let sp = parent.regs.get_sp();
        let (ret, sp) = unsafe { swap(arg, sp) };
        // the parent is cached as the last env which maybe not correct
        // we need to update it here after resume back!, but the self
        // is always the last context, so we need to get the current context
        // to get the correct parent here.
        let parent = unsafe { &mut *self.parent };
        parent.regs.set_sp(sp);
        ret
    }

    /// same as swap, but used for resume to link the ret address
    #[inline]
    pub fn swap_resume(&mut self, arg: usize) -> usize {
        // we already finish the context stack push
        // self is just the target generator

        // switch to new context, always use the top ctx's reg
        // for normal generator self.context.parent == self.context
        // for coroutine self.context.parent == top generator context
        // assert!(!self.parent.is_null());
        let top = unsafe { &mut *self.parent };
        // save current generator context on stack
        let env = ContextStack::current();

        env.push_context(self);
        top.regs.restore_context();

        let base = top.stack.end();
        let sp = top.regs.get_sp();
        let (ret, sp) = unsafe { swap_link(arg, sp, base) };

        // when come back it maybe not the same generator!!
        // note that the target is alredy popped up
        let top = unsafe { &mut *self.parent };
        top.regs.set_sp(unsafe { ::std::mem::transmute(sp) });
        ret
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
