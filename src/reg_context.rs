use crate::detail::{initialize_call_frame, swap_registers, InitFn, Registers};
use crate::stack::Stack;

#[derive(Debug)]
pub struct RegContext {
    /// Hold the registers while the task or scheduler is suspended
    pub(crate) regs: Registers,
}

impl RegContext {
    /// Create a fresh, all-zero context.
    ///
    /// This is only valid as the *save* target (`out_context`) of [`swap`]:
    /// resuming into it would restore an all-zero register set and jump to
    /// address 0. Use [`init_with`] (or a previous `swap`) to turn it into a
    /// context that can be resumed.
    pub fn empty() -> RegContext {
        RegContext {
            regs: Registers::new(),
        }
    }

    /// Prefetch the saved stack pointer into the cache.
    ///
    /// Only meaningful once the context has been initialized (via [`init_with`])
    /// or has saved a running context: on a fresh [`empty`] context the saved
    /// stack pointer is null. The prefetch instruction itself does not fault on
    /// an invalid address.
    #[inline]
    pub fn prefetch(&self) {
        self.regs.prefetch();
    }

    /// Create a new context, only used in tests
    #[cfg(test)]
    fn new(init: InitFn, arg: usize, start: *mut usize, stack: &Stack) -> RegContext {
        let mut ctx = RegContext::empty();
        ctx.init_with(init, arg, start, stack);
        ctx
    }

    /// init the generator register
    #[inline]
    pub fn init_with(&mut self, init: InitFn, arg: usize, start: *mut usize, stack: &Stack) {
        // Save and then immediately load the current context,
        // we will modify it to call the given function when restored back
        initialize_call_frame(&mut self.regs, init, arg, start, stack);
    }

    /// Switch contexts.
    ///
    /// Suspend the current execution context and resume another by saving the
    /// register values of the executing thread into `out_context`, then
    /// loading the registers from `in_context`.
    ///
    /// The caller must ensure that:
    /// - `in_context` holds a previously saved or initialized register set.
    ///   Passing a fresh [`empty`] context resumes from an all-zero register
    ///   set, which is undefined behavior.
    /// - `out_context` and `in_context` do not alias.
    #[inline]
    pub fn swap(out_context: &mut RegContext, in_context: &RegContext) {
        unsafe { swap_registers(&mut out_context.regs, &in_context.regs) }
    }

    /// Load the context and switch. This function will never return.
    ///
    /// The current context is saved into a scratch buffer that is immediately
    /// discarded: execution only continues in `to_context`.
    ///
    /// The caller must ensure that `to_context` holds a previously saved or
    /// initialized register set.
    #[inline]
    #[cfg(test)]
    pub fn load(to_context: &RegContext) {
        let mut cur = Registers::new();
        let regs: &Registers = &to_context.regs;

        unsafe { swap_registers(&mut cur, regs) }
    }
}

#[cfg(test)]
mod test {
    use std::mem::transmute;

    use crate::reg_context::RegContext;
    use crate::stack::Stack;

    const MIN_STACK: usize = 1024;

    fn init_fn_impl(arg: usize, f: *mut usize) -> ! {
        let func: fn() = unsafe { transmute(f) };
        func();

        let ctx: &RegContext = unsafe { &*std::ptr::with_exposed_provenance(arg) };
        RegContext::load(ctx);

        unreachable!("Should never come back");
    }

    #[cfg(target_arch = "x86_64")]
    extern "sysv64" fn init_fn(arg: usize, f: *mut usize) -> ! {
        init_fn_impl(arg, f)
    }

    #[cfg(target_arch = "x86")]
    extern "C" fn init_fn(arg: usize, f: *mut usize) -> ! {
        init_fn_impl(arg, f)
    }

    #[cfg(target_arch = "aarch64")]
    extern "C" fn init_fn(arg: usize, f: *mut usize) -> ! {
        init_fn_impl(arg, f)
    }

    #[cfg(target_arch = "loongarch64")]
    extern "C" fn init_fn(arg: usize, f: *mut usize) -> ! {
        init_fn_impl(arg, f)
    }

    #[cfg(target_arch = "riscv64")]
    extern "C" fn init_fn(arg: usize, f: *mut usize) -> ! {
        init_fn_impl(arg, f)
    }

    #[cfg(target_arch = "powerpc64")]
    extern "C" fn init_fn(arg: usize, f: *mut usize) -> ! {
        init_fn_impl(arg, f)
    }

    #[cfg(target_arch = "s390x")]
    extern "C" fn init_fn(arg: usize, f: *mut usize) -> ! {
        init_fn_impl(arg, f)
    }

    #[cfg(target_arch = "arm")]
    extern "aapcs" fn init_fn(arg: usize, f: *mut usize) -> ! {
        init_fn_impl(arg, f)
    }

    #[test]
    fn test_swap_context() {
        static mut VAL: bool = false;
        let mut cur = RegContext::empty();

        fn callback() {
            unsafe { VAL = true };
        }

        let stk = Stack::new(MIN_STACK);
        let ctx = RegContext::new(
            init_fn,
            &cur as *const _ as usize,
            callback as *mut usize,
            &stk,
        );

        RegContext::swap(&mut cur, &ctx);
        unsafe {
            assert!(VAL);
        }
    }
}
