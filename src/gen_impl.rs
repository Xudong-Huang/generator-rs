//! # generator
//!
//! Rust generator implementation
//!

use std::fmt;
use std::panic;
use std::thread;
use std::any::Any;
use std::marker::PhantomData;

use scope::Scope;
use stack::StackPointer;
use reg_context::RegContext;
use rt::{Context, ContextStack, Error};
use no_drop::{decode_usize, encode_usize, NoDrop};

// default stack size, in usize
// windows has a minimal size as 0x4a8!!!!
pub const DEFAULT_STACK_SIZE: usize = 0x1000;

/// Generator helper
pub struct Gn<A> {
    dummy: PhantomData<A>,
}

/// the generator type
pub type Generator<'a, A, T> = Box<GeneratorImpl<'a, A, T>>;
unsafe impl<A, T> Send for GeneratorImpl<'static, A, T> {}

impl<A> Gn<A> {
    /// create a scoped generator with default stack size
    pub fn new_scoped<'a, T, F>(f: F) -> Generator<'a, A, T>
    where
        F: FnOnce(Scope<A, T>) -> T + 'a,
        T: 'a,
        A: 'a,
    {
        Self::new_scoped_opt(DEFAULT_STACK_SIZE, f)
    }

    /// create a scoped generator with specified stack size
    pub fn new_scoped_opt<'a, T, F>(size: usize, f: F) -> Generator<'a, A, T>
    where
        F: FnOnce(Scope<A, T>) -> T + 'a,
        T: 'a,
        A: 'a,
    {
        let mut g = GeneratorImpl::<A, T>::new(size);
        let scope = g.get_scope();
        g.init(move || f(scope));
        g
    }
}

impl<A: Any> Gn<A> {
    /// create a new generator with default stack size
    pub fn new<'a, T: Any, F>(f: F) -> Generator<'a, A, T>
    where
        F: FnOnce() -> T + 'a,
    {
        Self::new_opt(DEFAULT_STACK_SIZE, f)
    }

    /// create a new generator with specified stack size
    pub fn new_opt<'a, T: Any, F>(size: usize, f: F) -> Generator<'a, A, T>
    where
        F: FnOnce() -> T + 'a,
    {
        let mut g = GeneratorImpl::<A, T>::new(size);
        g.init_context();
        g.init(f);
        g
    }
}

/// `GeneratorImpl`
pub struct GeneratorImpl<'a, A, T> {
    // run time context
    context: Context,
    // save the input
    para: Option<A>,
    // save the output
    // no need to save here, yield should return it
    ret: Option<T>,
    // phantom
    phantom: PhantomData<&'a u8>,
}

impl<'a, A: Any, T: Any> GeneratorImpl<'a, A, T> {
    /// create a new generator with default stack size
    pub fn init_context(&mut self) {
        self.context.para = &mut self.para as &mut Any;
        self.context.ret = &mut self.ret as &mut Any;
    }
}

impl<'a, A, T> GeneratorImpl<'a, A, T> {
    /// create a new generator with specified stack size
    pub fn new(size: usize) -> Box<Self> {
        Box::new(GeneratorImpl {
            para: None,
            ret: None,
            context: Context::new(size),
            phantom: PhantomData,
        })
    }

    /// prefech the generator into cache
    #[inline]
    pub fn prefetch(&self) {
        self.context.regs.prefetch();
    }

    /// get the scope object
    pub fn get_scope(&mut self) -> Scope<A, T> {
        Scope::new(&mut self.para, &mut self.ret)
    }

    /// prefech the generator into cache
    #[inline]
    fn return_wrap<F: FnOnce() + 'a>(&mut self, f: F) {
        let stk = &self.context.stack;
        self.context.regs.init_with(gen_wrapper::<F, A>, stk);

        // Transfer environment to the callee.
        let _f = NoDrop::new(f);
        let _arg = unsafe { encode_usize(&_f) };
        //  self.resume_gen()
        // for the first time, the arg is f that transfer to the callee stack
        //  let stack_ptr = arch::swap_link(encode_usize(&f), stack_ptr, stack.base()).1;
    }

    /// init a heap based generator
    // it's can be used to re-init a 'done' generator before it's get dropped
    pub fn init<F: FnOnce() -> T + 'a>(&mut self, f: F)
    where
        T: 'a,
    {
        // make sure the last one is finished
        if self.context._ref == 0 {
            unsafe {
                self.cancel();
            }
        }

        // init ctx parent to itself, this would be the new top
        self.context.parent = &mut self.context;

        // init the ref to 0 means that it's ready to start
        self.context._ref = 0;

        let ret = &mut self.ret as *mut _;
        let contex = &mut self.context as *mut Context;
        self.return_wrap(move || {
            let r = f();
            let ret = unsafe { &mut *ret };
            let _ref = unsafe { (*contex)._ref };
            if _ref == 0xf {
                *ret = None; // this is a done return
            } else {
                *ret = Some(r); // normal return
            }
        });
    }

    /// resume the generator
    #[inline]
    fn resume_gen(&mut self) {
        let env = ContextStack::current();
        // get the current regs
        let cur = &mut env.top().regs;

        // switch to new context, always use the top ctx's reg
        // for normal generator self.context.parent == self.context
        // for coroutine self.context.parent == top generator context
        assert!(!self.context.parent.is_null());
        let top = unsafe { &mut *self.context.parent };

        // save current generator context on stack
        env.push_context(&mut self.context);

        // swap to the generator
        RegContext::swap_link(cur, &mut top.regs, top.stack.end(), 0);

        // comes back, check the panic status
        // this would propagate the panic until root context
        // if it's a coroutine just stop propagate
        if !self.context.local_data.is_null() {
            return;
        }

        if let Some(err) = self.context.err.take() {
            // pass the error to the parent until root
            panic::resume_unwind(err);
        }
    }

    #[inline]
    fn is_started(&self) -> bool {
        true
    }

    /// prepare the para that passed into generator before send
    #[inline]
    pub fn set_para(&mut self, para: A) {
        self.para = Some(para);
    }

    /// set the generator local data
    #[inline]
    pub fn set_local_data(&mut self, data: *mut u8) {
        self.context.local_data = data;
    }

    /// get the generator local data
    #[inline]
    pub fn get_local_data(&self) -> *mut u8 {
        self.context.local_data
    }

    /// get the generator panic data
    #[inline]
    pub fn get_panic_data(&mut self) -> Option<Box<Any + Send>> {
        self.context.err.take()
    }

    /// resume the generator without touch the para
    /// you should call `set_para` before this method
    #[inline]
    pub fn resume(&mut self) -> Option<T> {
        if self.is_done() {
            return None;
        }

        // every time we call the function, increase the ref count
        // yiled will decrease it and return will not
        self.context._ref += 1;
        self.resume_gen();

        self.ret.take()
    }

    /// `raw_send`
    #[inline]
    pub fn raw_send(&mut self, para: Option<A>) -> Option<T> {
        if self.is_done() {
            return None;
        }

        // this is the passed in value of the send primitive
        // the yield part would read out this value in the next round
        self.para = para;

        // every time we call the function, increase the ref count
        // yiled will decrease it and return will not
        self.context._ref += 1;
        self.resume_gen();

        self.ret.take()
    }

    /// send interface
    pub fn send(&mut self, para: A) -> T {
        let ret = self.raw_send(Some(para));
        ret.expect("send got None return")
    }

    /// cancel the generator without any check
    #[inline]
    unsafe fn raw_cancel(&mut self) {
        // tell the func to panic
        // so that we can stop the inner func
        self.context._ref = 2;
        // save the old panic hook, we don't want to print anything for the Cancel
        let old = ::std::panic::take_hook();
        ::std::panic::set_hook(Box::new(|_| {}));
        self.resume_gen();
        ::std::panic::set_hook(old);
    }

    /// cancel the generator
    /// this will trigger a Cancel panic, it's unsafe in that you must care about the resource
    pub unsafe fn cancel(&mut self) {
        if self.is_done() {
            return;
        }

        // consume the fun if it's not started
        // TODO: when init is called, it's always started!
        if !self.is_started() {
            self.context._ref = 1;
        } else {
            self.raw_cancel();
        }
    }

    /// is finished
    #[inline]
    pub fn is_done(&self) -> bool {
        self.is_started() && (self.context._ref & 0x3) != 0
    }

    /// get stack total size and used size in word
    pub fn stack_usage(&self) -> (usize, usize) {
        (
            self.context.stack.size(),
            self.context.stack.get_used_size(),
        )
    }
}

impl<'a, A, T> Drop for GeneratorImpl<'a, A, T> {
    fn drop(&mut self) {
        // when the thread is already panic, do nothing
        if thread::panicking() {
            return;
        }

        if !self.is_started() {
            // not started yet, just drop the gen
            return;
        }

        if !self.is_done() {
            warn!("generator is not done while drop");
            unsafe { self.raw_cancel() }
        }

        assert!(self.is_done());

        let (total_stack, used_stack) = self.stack_usage();
        if used_stack < total_stack {
            // here we should record the stack in the class
            // next time will just use
            // set_stack_size::<F>(used_stack);
        } else {
            error!("stack overflow detected!");
            panic!(Error::StackErr);
        }
    }
}

impl<'a, T> Iterator for GeneratorImpl<'a, (), T> {
    type Item = T;
    // The 'Iterator' trait only requires the 'next' method to be defined. The
    // return type is 'Option<T>', 'None' is returned when the 'Iterator' is
    // over, otherwise the next value is returned wrapped in 'Some'
    fn next(&mut self) -> Option<T> {
        self.resume()
    }
}

impl<'a, A, T> fmt::Debug for GeneratorImpl<'a, A, T> {
    #[cfg(nightly)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use std::intrinsics::type_name;
        write!(
            f,
            "Generator<{}, Output={}> {{ ... }}",
            unsafe { type_name::<A>() },
            unsafe { type_name::<T>() }
        )
    }

    #[cfg(not(nightly))]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Generator {{ ... }}")
    }
}

// the init function passed to reg_context
//
// the first arg is the env data
// the second arg is the peer stack pointer
fn gen_wrapper<'a, F: FnOnce() + 'a, Input>(env: usize, _sp: StackPointer) {
    fn check_err(cause: Box<Any + Send + 'static>) {
        match cause.downcast_ref::<Error>() {
            // this is not an error at all, ignore it
            Some(_e @ &Error::Cancel) => return,
            _ => {}
        }
        error!("set panicked inside generator");
        ContextStack::current().top().err = Some(cause);
    }

    // Retrieve our environment from the caller and return control to it.
    let f: F = unsafe { decode_usize(env) };
    // the first invoke doesn't necessarily pass in anything
    // just for init and return to the parent caller
    // let (data, stack_ptr) = arch::swap(0, stack_ptr);

    // the first data is only for start the generator and is not used
    // TODO: how to use the second returned data here?
    // actually it's saved in the generator
    // let data: Input = yield_now();
    // See the second half of Yielder::suspend_bare.
    // let input: Input = decode_usize(data);
    // Run the body of the generator.
    // let yielder = Yielder::new(stack_ptr);

    // we can't panic inside the generator context
    // need to propagate the panic to the main thread
    if let Err(cause) = panic::catch_unwind(panic::AssertUnwindSafe(f)) {
        check_err(cause);
    }

    // return to caller by assembly code!
    // after finished the control will be in caller's stack
}
