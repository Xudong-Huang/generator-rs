use stack::{Stack, StackPointer};
use detail::{initialize_call_frame, save_context, swap, swap_link, Registers};

// Hold the registers of the generator
// the most important register the stack pointer
#[derive(Debug)]
pub struct RegContext {
    regs: Registers,
}

// the first argument is passed in through swap/resume function
// usually this is the passed in functor
// the seconde argments is the target sp address
// this must be compatible with the interface that defined by
// assmbly swap functoin
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

    /// init the generator stack and registers
    #[inline]
    pub fn init_with(&mut self, init: InitFn, stack: &Stack) {
        // this would swap into the generator and then yield back to there
        // thus the registers will be updated accordingly
        unsafe { initialize_call_frame(&mut self.regs, init, stack) };
    }

    /// Switch execution contexts to another stack
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
    pub fn swap(src: &mut RegContext, dst: &mut RegContext, arg: usize) -> usize {
        unsafe { save_context(&mut src.regs, &mut dst.regs) };
        let sp = dst.regs.get_sp();
        let (ret, sp) = unsafe { swap(arg, sp) };
        dst.regs.set_sp(sp);
        ret
    }

    /// same as swap, but used for resume to link the ret address
    #[inline]
    pub fn swap_link(
        src: &mut RegContext,
        dst: &mut RegContext,
        base: *mut usize,
        arg: usize,
    ) -> usize {
        unsafe { save_context(&mut src.regs, &mut dst.regs) };
        let sp = dst.regs.get_sp();
        let (ret, sp) = unsafe { swap_link(arg, sp, base) };
        // if sp is None means the generator is finished
        dst.regs.set_sp(unsafe { ::std::mem::transmute(sp) });
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::transmute;
    const MIN_STACK: usize = 2 * 1024 * 1024;

    // this target funcion call
    // the argument is passed in through the first swap
    fn init_fn(env: usize, sp: StackPointer) {
        let func: fn(StackPointer) = unsafe { transmute(env) };
        func(sp);
        // after this will return to the caller
    }

    #[test]
    fn test_swap_context() {
        let mut cur = RegContext::empty();

        fn callback(sp: StackPointer) {
            // useless cur ctx
            let mut cur = RegContext::empty();
            // construct a dst ctx
            let mut dst = RegContext::empty();
            let mut out = 42;
            loop {
                dst.regs.set_sp(sp);
                let para = RegContext::swap(&mut cur, &mut dst, out);
                if para == 0 {
                    return;
                }
                out += 1;
                assert_eq!(para, out);
            }
        }

        let stk = Stack::new(MIN_STACK);
        // TODO: how to to pass the callback to ctx?
        let mut ctx = RegContext::empty();
        ctx.init_with(init_fn, &stk);

        // send the function to the generator
        let ret = RegContext::swap_link(&mut cur, &mut ctx, stk.end(), callback as usize);
        assert_eq!(ret, 42);
        let ret = RegContext::swap_link(&mut cur, &mut ctx, stk.end(), ret + 1);
        assert_eq!(ret, 43);
        let ret = RegContext::swap_link(&mut cur, &mut ctx, stk.end(), ret + 1);
        assert_eq!(ret, 44);
        // finish the generator
        RegContext::swap_link(&mut cur, &mut ctx, stk.end(), 0);
        let sp = unsafe { ctx.regs.get_sp().offset(0) as usize };
        assert_eq!(sp, 0);
    }
}
