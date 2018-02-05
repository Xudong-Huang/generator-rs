use detail::{initialize_call_frame, swap_registers, Registers};
use stack::{Stack, StackPointer};

#[derive(Debug)]
pub struct RegContext {
    /// Hold the registers while the task or scheduler is suspended
    regs: Registers,
}

// the first argument is passed in through swap(resume) function
// the seconde argments is the target sp address
// this must keep the same interface as swap defined
pub type InitFn = unsafe fn(usize, StackPointer);

impl RegContext {
    pub fn empty() -> RegContext {
        RegContext {
            regs: Registers::new(),
        }
    }

    #[inline]
    pub fn prefetch(&self) {
        self.regs.prefetch();
    }

    /// Create a new context
    #[allow(dead_code)]
    pub fn new(init: InitFn, stack: &Stack) -> RegContext {
        let mut ctx = RegContext::empty();
        ctx.init_with(init, stack);
        ctx
    }

    /// init the generator register
    #[inline]
    pub fn init_with(&mut self, init: InitFn, stack: &Stack) {
        // Save and then immediately load the current context,
        // which we will then modify to call the given function when restoredtack
        unsafe { initialize_call_frame(&mut self.regs, init, stack) };
    }

    /// Switch contexts
    ///
    /// Suspend the current execution context and resume another by
    /// saving the registers values of the executing thread to a Context
    /// then loading the registers from a previously saved Context.
    #[inline]
    pub fn swap(out_context: &mut RegContext, in_context: &RegContext) {
        // debug!("swapping contexts");
        let out_regs: &mut Registers = match *out_context {
            RegContext {
                regs: ref mut r, ..
            } => r,
        };
        let in_regs: &Registers = match *in_context {
            RegContext { regs: ref r, .. } => r,
        };

        // debug!("register raw swap");

        unsafe { swap_registers(out_regs, in_regs) }
    }

    /// Load the context and switch. This function will never return.
    #[inline]
    #[allow(dead_code)]
    pub fn load(to_context: &RegContext) {
        let mut cur = Registers::new();
        let regs: &Registers = &to_context.regs;

        unsafe { swap_registers(&mut cur, regs) }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::mem::transmute;
    const MIN_STACK: usize = 2 * 1024 * 1024;

    // this target funcion call
    // the argument is passed in through the first swap
    fn init_fn(env: usize, _sp: StackPointer) {
        let func: fn() = unsafe { transmute(env) };
        func();
        // after this will return to the caller
    }

    #[test]
    fn test_swap_context() {
        static mut VAL: bool = false;
        let mut cur = RegContext::empty();

        // fn callback() {
        //     unsafe {
        //         VAL = true;
        //     }
        // }

        let stk = Stack::new(MIN_STACK);
        // TODO: how to to pass the callback to ctx?
        let ctx = RegContext::new(init_fn, &stk);

        RegContext::swap(&mut cur, &ctx);
        unsafe {
            assert!(VAL);
        }
    }
}
