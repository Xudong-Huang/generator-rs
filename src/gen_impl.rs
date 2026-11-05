//! # generator
//!
//! Rust generator implementation
//!

use crate::detail::gen_init;
use crate::reg_context::RegContext;
use crate::rt::{
    Context, ContextStack, Error, REF_ACTIVE, REF_CANCELLED, REF_IDLE, REF_STATE_MASK,
};
use crate::scope::Scope;
use crate::stack::{overflow, Func, Stack, StackBox};

use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::panic;
use std::thread;

/// The default stack size for generators.
///
/// Expressed in `usize` words (not bytes): on a 64-bit target this is 4096
/// words, i.e. 32 KiB; on a 32-bit target it is 8192 words, also 32 KiB. The
/// public `*_opt` constructors take their `size` argument in the same unit
/// (`usize` words).
// windows has a minimal size as 0x4a8!!!!
pub const DEFAULT_STACK_SIZE: usize = 0x8000 / std::mem::size_of::<usize>();

#[inline]
pub(crate) fn unlikely(b: bool) -> bool {
    if b {
        std::hint::cold_path()
    }
    b
}

/// the generator obj type, the functor passed to it must be Send
pub struct GeneratorObj<'a, A, T, const LOCAL: bool> {
    gen: StackBox<GeneratorImpl<'a, A, T>>,
}

/// the generator type, the functor passed to it must be Send
///
/// `A` is an input to the generator body, so the type is invariant in it:
/// the input lifetime cannot be shortened after the generator is created.
///
/// A `Generator` is `Send` when `A` and `T` are `Send`, but the compiler
/// cannot see which values the body keeps on its stack across a yield. Only
/// move a suspended generator to another thread if those values are `Send`
/// and no aliases of them remain on the originating thread; see the notes on
/// the `Send` impl below.
///
/// ```compile_fail
/// use generator::Generator;
///
/// fn shorten<'b>(
///     g: Generator<'static, &'static str, u32>,
/// ) -> Generator<'static, &'b str, u32> {
///     g
/// }
/// ```
pub type Generator<'a, A, T> = GeneratorObj<'a, A, T, false>;

// The stack handles are thread-affine (`Rc`): they all live in the generator's
// own stack allocation, so moving the generator moves the access to them too
// and the non-atomic counts are never touched from several threads at once.
//
// `A: Send`, `T: Send` and `F: Send` (enforced by the constructors) are not
// sufficient on their own: the functor's `Send` bound only covers its
// captures, while a suspended generator's stack holds the body's live locals,
// which the type system cannot see. Moving a suspended generator to another
// thread is only safe when every value alive on its stack across a yield is
// `Send` and no alias of such a value is left on the originating thread - the
// same rule a future must obey to be `Send`.
unsafe impl<A: Send, T: Send> Send for Generator<'static, A, T> {}

impl<'a, A, T> Generator<'a, A, T> {
    /// init a heap based generator with scoped closure
    pub fn scoped_init<F>(&mut self, f: F)
    where
        for<'scope> F: FnOnce(Scope<'scope, 'a, A, T>) -> T + Send + 'a,
        T: Send + 'a,
        A: Send + 'a,
    {
        self.gen.scoped_init(f);
    }

    /// init a heap based generator
    // it can be used to re-init a 'done' generator before it is dropped
    pub fn init_code<F: FnOnce() -> T + Send + 'a>(&mut self, f: F)
    where
        T: Send + 'a,
    {
        self.gen.init_code(f);
    }
}

/// the local generator type, can't Send
///
/// Unlike [`Generator`], it accepts non-`Send` closures, but the result
/// cannot be moved to another thread:
///
/// ```compile_fail
/// fn assert_send<T: Send>(_: T) {}
///
/// let g = generator::Gn::<()>::new_scoped_local(|_s| 1u32);
/// assert_send(g); // `LocalGenerator` is not `Send`
/// ```
pub type LocalGenerator<'a, A, T> = GeneratorObj<'a, A, T, true>;

impl<'a, A, T> LocalGenerator<'a, A, T> {
    /// init a heap based generator with scoped closure
    pub fn scoped_init<F>(&mut self, f: F)
    where
        for<'scope> F: FnOnce(Scope<'scope, 'a, A, T>) -> T + 'a,
        T: 'a,
        A: 'a,
    {
        self.gen.scoped_init(f);
    }
}

impl<'a, A, T, const LOCAL: bool> GeneratorObj<'a, A, T, LOCAL> {
    /// Constructs a `Generator` from a raw pointer.
    ///
    /// # Safety
    ///
    /// `raw` must be a non-null pointer returned by
    /// [`into_raw`](Self::into_raw), and it must be reconstructed only once:
    ///
    /// - reconstructing a pointer twice (or while another handle still owns
    ///   it) causes a double free;
    /// - the pointer must be reconstructed with the same `A`, `T` and
    ///   `LOCAL` parameters it was created with. In particular, a pointer
    ///   obtained from a [`LocalGenerator`] must never be reconstructed as a
    ///   [`Generator`]: the local generator's functor was not required to be
    ///   `Send`, and that requirement cannot be recovered after the closure
    ///   was type-erased;
    /// - the reconstructed `'a` must be the lifetime the generator was
    ///   created with. The erased functor may borrow data for `'a`, and this
    ///   lifetime is what makes the borrow checker keep that data alive until
    ///   the generator (and its destructor) is gone; choosing a different
    ///   lifetime can let the destructor run after the borrowed data has been
    ///   freed.
    #[inline]
    pub unsafe fn from_raw(raw: *mut usize) -> Self {
        GeneratorObj {
            gen: StackBox::from_raw(raw as *mut GeneratorImpl<'a, A, T>),
        }
    }

    /// Consumes the `Generator`, returning a wrapped raw pointer.
    ///
    /// The pointer owns the generator and its stack. It must be reclaimed
    /// exactly once with [`from_raw`](Self::from_raw) on the matching
    /// generator type, or leaked deliberately.
    #[inline]
    pub fn into_raw(self) -> *mut usize {
        let ret = self.gen.as_ptr() as *mut usize;
        std::mem::forget(self);
        ret
    }

    /// prefetch the generator into cache
    #[inline]
    pub fn prefetch(&self) {
        self.gen.prefetch();
    }

    /// prepare the para that passed into generator before send
    #[inline]
    pub fn set_para(&mut self, para: A) {
        self.gen.set_para(para);
    }

    /// set the generator local data
    #[inline]
    pub fn set_local_data(&mut self, data: *mut u8) {
        self.gen.set_local_data(data);
    }

    /// get the generator local data
    #[inline]
    pub fn get_local_data(&self) -> *mut u8 {
        self.gen.get_local_data()
    }

    /// get the generator panic data
    ///
    /// This only returns something for a coroutine-style generator (one whose
    /// local data is set): a normal generator re-raises its panic from
    /// `resume` instead, and the error is consumed there.
    #[inline]
    pub fn get_panic_data(&mut self) -> Option<Box<dyn Any + Send>> {
        self.gen.get_panic_data()
    }

    /// resume the generator without touch the para
    /// you should call `set_para` before this method
    #[inline]
    pub fn resume(&mut self) -> Option<T> {
        self.gen.resume()
    }

    /// resume the generator, passing `para` as the value the next
    /// `get_yield` call returns; returns `None` once the generator is done
    #[inline]
    pub fn raw_send(&mut self, para: Option<A>) -> Option<T> {
        self.gen.raw_send(para)
    }

    /// resume the generator with `para` and return the next yielded value
    ///
    /// # Panics
    ///
    /// Panics if the generator finishes instead of yielding a value.
    pub fn send(&mut self, para: A) -> T {
        self.gen.send(para)
    }

    /// Cancel the generator.
    ///
    /// A suspended generator is resumed with a `Cancel` panic at its yield
    /// point, which unwinds its body and finishes it. A generator that has
    /// not started is finished without running its body: its closure is
    /// dropped. Cancelling a generator that is already done does nothing.
    pub fn cancel(&mut self) {
        self.gen.cancel()
    }

    /// is finished
    #[inline]
    pub fn is_done(&self) -> bool {
        self.gen.is_done()
    }

    /// Get the stack total size and used size, in `usize` words.
    ///
    /// `used` is exact only when the generator was created with an odd stack
    /// size, which enables full usage tracking. Otherwise only a small sample
    /// at the bottom of the stack is checked and `used` is an upper bound
    /// close to the total.
    pub fn stack_usage(&self) -> (usize, usize) {
        self.gen.stack_usage()
    }
}

impl<T, const LOCAL: bool> Iterator for GeneratorObj<'_, (), T, LOCAL> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.resume()
    }
}

impl<A, T, const LOCAL: bool> fmt::Debug for GeneratorObj<'_, A, T, LOCAL> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Generator<{}, Output={}, Local={}> {{ ... }}",
            std::any::type_name::<A>(),
            std::any::type_name::<T>(),
            LOCAL
        )
    }
}

/// Generator helper
pub struct Gn<A = ()> {
    dummy: PhantomData<A>,
}

impl<A> Gn<A> {
    /// create a scoped generator with default stack size
    ///
    /// The closure must be `Send`, so it cannot capture non-`Send` values;
    /// use [`new_scoped_local`](Self::new_scoped_local) for those.
    ///
    /// ```compile_fail
    /// use generator::Gn;
    /// use std::rc::Rc;
    ///
    /// let rc = Rc::new(0);
    /// let _g = Gn::<()>::new_scoped(move |_s| drop(rc)); // `Rc` is not `Send`
    /// ```
    pub fn new_scoped<'a, T, F>(f: F) -> Generator<'a, A, T>
    where
        for<'scope> F: FnOnce(Scope<'scope, 'a, A, T>) -> T + Send + 'a,
        T: Send + 'a,
        A: Send + 'a,
    {
        Self::new_scoped_opt(DEFAULT_STACK_SIZE, f)
    }

    /// create a scoped local generator with default stack size
    pub fn new_scoped_local<'a, T, F>(f: F) -> LocalGenerator<'a, A, T>
    where
        F: FnOnce(Scope<A, T>) -> T + 'a,
        T: 'a,
        A: 'a,
    {
        Self::new_scoped_opt_local(DEFAULT_STACK_SIZE, f)
    }

    /// create a scoped generator with specified stack size
    ///
    /// `size` is the stack size in `usize` words (not bytes), matching
    /// [`DEFAULT_STACK_SIZE`].
    pub fn new_scoped_opt<'a, T, F>(size: usize, f: F) -> Generator<'a, A, T>
    where
        for<'scope> F: FnOnce(Scope<'scope, 'a, A, T>) -> T + Send + 'a,
        T: Send + 'a,
        A: Send + 'a,
    {
        let mut gen = GeneratorImpl::<A, T>::new(Stack::new(size));
        gen.scoped_init(f);
        Generator { gen }
    }

    /// create a scoped local generator with specified stack size
    ///
    /// `size` is the stack size in `usize` words (not bytes), matching
    /// [`DEFAULT_STACK_SIZE`].
    pub fn new_scoped_opt_local<'a, T, F>(size: usize, f: F) -> LocalGenerator<'a, A, T>
    where
        F: FnOnce(Scope<A, T>) -> T + 'a,
        T: 'a,
        A: 'a,
    {
        let mut gen = GeneratorImpl::<A, T>::new(Stack::new(size));
        gen.scoped_init(f);
        LocalGenerator { gen }
    }
}

impl<A: Any> Gn<A> {
    /// create a new generator with default stack size
    #[allow(clippy::new_ret_no_self)]
    #[deprecated(since = "0.6.18", note = "please use `scope` version instead")]
    pub fn new<'a, T: Any, F>(f: F) -> Generator<'a, A, T>
    where
        F: FnOnce() -> T + Send + 'a,
    {
        Self::new_opt(DEFAULT_STACK_SIZE, f)
    }

    /// create a new generator with specified stack size
    ///
    /// `size` is the stack size in `usize` words (not bytes), matching
    /// [`DEFAULT_STACK_SIZE`].
    // the `may` library use this API so we can't deprecated it yet.
    pub fn new_opt<'a, T: Any, F>(size: usize, f: F) -> Generator<'a, A, T>
    where
        F: FnOnce() -> T + Send + 'a,
    {
        let mut gen = GeneratorImpl::<A, T>::new(Stack::new(size));
        gen.init_context();
        gen.init_code(f);
        Generator { gen }
    }
}

/// `GeneratorImpl`
#[repr(C)]
struct GeneratorImpl<'a, A, T> {
    // run time context
    context: Context,
    // stack
    stack: Stack,
    // save the input
    para: Option<A>,
    // save the output
    ret: Option<T>,
    // boxed functor
    f: Option<Func>,
    // phantom lifetime
    phantom: PhantomData<&'a T>,
    // `A` is an input to the erased closure (`Scope::get_yield` consumes it),
    // so it must be invariant: `para: Option<A>` alone only makes the struct
    // covariant, which would let safe code shorten a `'static` input lifetime
    // and hand a short-lived value to a body compiled for `'static`.
    // `fn(A) -> A` gives that invariance without affecting auto traits or
    // drop order. `T` is output only and may stay covariant.
    input: PhantomData<fn(A) -> A>,
}

impl<A: Any, T: Any> GeneratorImpl<'_, A, T> {
    /// point the deprecated `Context` input/output fields at this generator's
    /// slots, so `yield_`/`get_yield` can reach them without a `Scope`
    fn init_context(&mut self) {
        unsafe {
            std::ptr::write(
                self.context.para.as_mut_ptr(),
                &mut self.para as &mut dyn Any,
            );
            std::ptr::write(self.context.ret.as_mut_ptr(), &mut self.ret as &mut dyn Any);
        }
    }
}

impl<'a, A, T> GeneratorImpl<'a, A, T> {
    /// create a new generator backed by `stack`
    fn new(mut stack: Stack) -> StackBox<Self> {
        // the box keeps the stack alive; the mapping is released with the
        // last `Stack` handle
        let stack_box = stack.alloc_uninit_box::<GeneratorImpl<'a, A, T>>();
        stack_box.init(GeneratorImpl {
            para: None,
            stack,
            ret: None,
            f: None,
            context: Context::new(),
            phantom: PhantomData,
            input: PhantomData,
        })
    }

    /// prefetch the generator into cache
    #[inline]
    pub fn prefetch(&self) {
        self.context.regs.prefetch();
    }

    /// Reset the run state before installing a new body.
    ///
    /// The input and output slots are cleared here, before `scoped_init`
    /// creates a `Scope` that borrows them: writing through `&mut self` after
    /// that point would invalidate the scope's references. Returns the raw
    /// pointer the body's return value is written to.
    fn prepare_init(&mut self) -> *mut Option<T> {
        // Install the overflow handler before the generator stack is used for
        // the first time. The bootstrap frame runs before `gen_init_impl` gets
        // a chance to install it, so a stack too small for that frame must
        // fault into the handler instead of crashing the process.
        overflow::init_once();

        // make sure the last one is finished
        if self.f.is_none() && self.context._ref == REF_IDLE {
            self.cancel();
        }

        // From here on the generator has no live run. Mark it done before the
        // fallible steps below: dropping a previous functor runs user code,
        // and allocating the new one can panic when it does not fit into the
        // stack. A caught panic must not leave the generator resumable with
        // stale registers.
        self.context.parent = &mut self.context;
        self.context._ref = REF_ACTIVE;

        let _ = self.f.take();

        // The new run must not observe input, output or panic state left over
        // from the previous one: `para` may hold a value the old body never
        // consumed, and a coroutine's `err` is only taken by `get_panic_data`.
        self.para = None;
        self.ret = None;
        self.context.err = None;

        &mut self.ret as *mut _
    }

    /// Install `f` as the generator body and point the registers at it.
    ///
    /// `ret` is the slot returned by [`prepare_init`](Self::prepare_init); it
    /// is captured before any `Scope` borrows that slot.
    fn install_code<F: FnOnce() -> T + 'a>(&mut self, ret: *mut Option<T>, f: F)
    where
        T: 'a,
    {
        // alloc the function on stack
        let func = StackBox::new_fn_once(&self.stack, move || {
            let r = f();
            unsafe { *ret = Some(r) };
        });

        // the function is installed, the generator is ready to start again
        self.f = Some(func);
        self.context._ref = REF_IDLE;

        let guard = (self.stack.begin() as usize, self.stack.end() as usize);
        self.context.stack_guard = guard;
        self.context.regs.init_with(
            gen_init,
            0,
            &mut self.f as *mut _ as *mut usize,
            &self.stack,
        );
    }

    /// init a heap based generator with scoped closure
    fn scoped_init<F>(&mut self, f: F)
    where
        for<'scope> F: FnOnce(Scope<'scope, 'a, A, T>) -> T + 'a,
        T: 'a,
        A: 'a,
    {
        use std::mem::transmute;

        // Clear the slots, and take the return-value pointer, before the new
        // `Scope` borrows them: clearing after `Scope::new` would write into
        // places the scope already mutably borrows, and a pointer taken after
        // it would be invalidated when the body first uses the scope.
        let ret = self.prepare_init();

        // SAFETY: the scope borrows this generator's own para/ret slots,
        // which stay at a fixed address for as long as the generator exists.
        // The transmute only replaces the borrow's lifetime with the
        // generator's `'a`; the `for<'scope>` bound on `F` keeps the scope
        // from escaping the generator body, and `install_code` stores the
        // closure on the generator's own stack.
        let scope: Scope<A, T> = unsafe { transmute(Scope::new(&mut self.para, &mut self.ret)) };
        self.install_code(ret, move || f(scope));
    }

    /// init a heap based generator
    // it can be used to re-init a 'done' generator before it is dropped
    fn init_code<F: FnOnce() -> T + 'a>(&mut self, f: F)
    where
        T: 'a,
    {
        let ret = self.prepare_init();
        self.install_code(ret, f);
    }

    /// resume the generator
    #[inline]
    fn resume_gen(&mut self) {
        let env = ContextStack::current();
        // get the current regs
        let cur = &mut env.top().regs;

        // switch to new context, always use the top context's reg
        // for normal generator self.context.parent == self.context
        // for coroutine self.context.parent == top generator context
        debug_assert!(!self.context.parent.is_null());
        let top = unsafe { &mut *self.context.parent };

        // save current generator context on stack
        env.push_context(&mut self.context);

        // swap to the generator
        RegContext::swap(cur, &top.regs);

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
        // when the f is consumed we think it's running
        self.f.is_none()
    }

    /// prepare the para that passed into generator before send
    #[inline]
    fn set_para(&mut self, para: A) {
        self.para = Some(para);
    }

    /// set the generator local data
    #[inline]
    fn set_local_data(&mut self, data: *mut u8) {
        self.context.local_data = data;
    }

    /// get the generator local data
    #[inline]
    fn get_local_data(&self) -> *mut u8 {
        self.context.local_data
    }

    /// get the generator panic data
    #[inline]
    fn get_panic_data(&mut self) -> Option<Box<dyn Any + Send>> {
        self.context.err.take()
    }

    /// resume the generator without touch the para
    /// you should call `set_para` before this method
    #[inline]
    fn resume(&mut self) -> Option<T> {
        if unlikely(self.is_done()) {
            return None;
        }

        // every time we call the function, increase the ref count
        // yield will decrease it and return will not
        self.context._ref += 1;
        self.resume_gen();

        self.ret.take()
    }

    /// resume the generator, passing `para` to the next `get_yield`
    #[inline]
    fn raw_send(&mut self, para: Option<A>) -> Option<T> {
        if unlikely(self.is_done()) {
            return None;
        }

        // this is the passed in value of the send primitive
        // the yield part would read out this value in the next round
        self.para = para;

        // every time we call the function, increase the ref count
        // yield will decrease it and return will not
        self.context._ref += 1;
        self.resume_gen();

        self.ret.take()
    }

    /// resume with `para` and return the next yielded value
    fn send(&mut self, para: A) -> T {
        let ret = self.raw_send(Some(para));
        ret.expect("generator finished instead of yielding")
    }

    /// cancel the generator without any check
    #[inline]
    fn raw_cancel(&mut self) {
        // tell the func to panic
        // so that we can stop the inner func
        self.context._ref = REF_CANCELLED;
        // NOTE: the `Cancel` control-flow panic raised inside the generator is
        // caught and silenced by `gen_init_impl`'s `catch_unwind_filter`, so we
        // must NOT touch the global panic hook here. Doing `panic::set_hook`
        // would race with other threads (the hook is process-global) and could
        // swallow unrelated panics or leave the hook in a broken state.
        // let old = panic::take_hook();
        // panic::set_hook(Box::new(|_| {}));
        self.resume_gen();
        // panic::set_hook(old);
    }

    /// Cancel the generator.
    ///
    /// A suspended run is resumed with a `Cancel` panic that unwinds its
    /// stack; a run that never started is dropped without being executed.
    fn cancel(&mut self) {
        if self.is_done() {
            return;
        }

        // consume the fun if it's not started
        if !self.is_started() {
            self.f.take();
            self.context._ref = REF_ACTIVE;
        } else {
            self.raw_cancel();
        }
    }

    /// is finished
    #[inline]
    fn is_done(&self) -> bool {
        self.is_started() && (self.context._ref & REF_STATE_MASK) != REF_IDLE
    }

    /// get stack total size and used size in word
    fn stack_usage(&self) -> (usize, usize) {
        (self.stack.size(), self.stack.get_used_size())
    }
}

impl<A, T> Drop for GeneratorImpl<'_, A, T> {
    fn drop(&mut self) {
        if self.context.overflowed {
            // The overflow handler yields out of the generator without
            // unwinding its frames, so the functor block allocated by
            // `init_code` is still reserved. Tell the stack bookkeeping not to
            // enforce the LIFO check for this box.
            self.stack.abandon();
        }

        // when the thread is already panic, do nothing
        if thread::panicking() {
            return;
        }

        if !self.is_started() {
            // not started yet, just drop the gen
            return;
        }

        if !self.is_done() {
            trace!("generator is not done while drop");
            self.raw_cancel()
        }

        assert!(self.is_done());

        let (total_stack, used_stack) = self.stack_usage();
        if used_stack < total_stack {
            // here we should record the stack in the class
            // next time will just use
            // set_stack_size::<F>(used_stack);
        } else if !self.context.overflowed {
            // Only surface the overflow here when the guard-page handler did
            // not already deliver it through `context.err`: a caller that
            // caught that error must be able to drop the generator normally.
            // This check is the fallback for a stack that was filled to the
            // bottom without faulting. The flag also stays set because the
            // sentinel this check relies on is destroyed by an overflow.
            error!("stack overflow detected!");
            panic::panic_any(Error::StackErr);
        }
    }
}

// Windows cannot reliably surface the guard-page fault as
// `EXCEPTION_STACK_OVERFLOW` from a probe frame (the kernel may consume the
// guard page and retry the access instead), so the overflow tests are only
// run where a real stack exhaustion can be triggered in-process.
#[cfg(test)]
#[cfg(all(not(target_os = "fuchsia"), not(windows)))]
mod test {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn test_overflow_marks_the_context() {
        // not a tail call, so release builds cannot turn it into a loop
        #[inline(never)]
        fn exhaust(n: usize) -> usize {
            let buf = [0u8; 512];
            std::hint::black_box(&buf);
            if n == usize::MAX {
                return 0;
            }
            let r = exhaust(n + 1);
            std::hint::black_box(r.wrapping_add(buf[0] as usize))
        }

        let mut g = Gn::<()>::new_opt(16, || exhaust(0));

        let result = catch_unwind(AssertUnwindSafe(|| g.next()));
        assert!(result.is_err());
        assert!(matches!(
            result.map_err(|err| *err.downcast::<Error>().unwrap()),
            Err(Error::StackErr)
        ));

        // the handler must remember that it reported the overflow, so that
        // dropping the generator does not panic a second time
        assert!(g.gen.context.overflowed);
    }
}
