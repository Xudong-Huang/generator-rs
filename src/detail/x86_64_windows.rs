// use detail::{align_down, mut_offset};
use reg_context::InitFn;
use stack::{Stack, StackPointer};

#[allow(dead_code)]
#[inline]
pub fn prefetch(data: *const usize) {
    unsafe {
        prefetch_asm(data);
    }
}

/// prefetch data
#[inline]
pub unsafe fn prefetch_asm(data: *const usize) {
    asm!("prefetcht1 $0"
             : // no output
             : "m"(*data)
             :
             : "volatile");
}

#[inline(always)]
pub unsafe extern "C" fn swap_registers(_out_regs: *mut Registers, _in_regs: *const Registers) {
    unimplemented!()
}

pub unsafe fn initialize_call_frame(regs: &mut Registers, fptr: InitFn, stack: &Stack) {
    #[naked]
    unsafe extern "C" fn trampoline_1() {
        asm!(
      r#"
        # This nop is here so that the initial swap doesn't return to the start
        # of the trampoline, which confuses the unwinder since it will look for
        # frame information in the previous symbol rather than this one. It is
        # never actually executed.
        nop

        # Stack unwinding in some versions of libunwind doesn't seem to like
        # 1-byte symbols, so we add a second nop here. This instruction isn't
        # executed either, it is only here to pad the symbol size.
        nop
      "#
      : : : : "volatile")
    }

    #[naked]
    unsafe extern "C" fn trampoline_2() {
        asm!(
      r#"
        # This nop is here so that the return address of the swap trampoline
        # doesn't point to the start of the symbol. This confuses gdb's backtraces,
        # causing them to think the parent function is trampoline_1 instead of
        # trampoline_2.
        nop

        # Call with the provided function
        call    *16(%rsp)

        # Restore the stack pointer of the parent context. No CFI adjustments
        # are needed since we have the same stack frame as trampoline_1.
        movq    (%rsp), %rsp

        # Restore frame pointer of the parent context.
        popq    %rbp

        # Clear the stack pointer. We can't call into this context any more once
        # the function has returned.
        xorq    %rdx, %rdx

        # Return into the parent context. Use `pop` and `jmp` instead of a `ret`
        # to avoid return address mispredictions (~8ns per `ret` on Ivy Bridge).
        popq    %rax
        jmpq    *%rax
      "#
      : : : : "volatile")
    }

    // We set up the stack in a somewhat special way so that to the unwinder it
    // looks like trampoline_1 has called trampoline_2, which has in turn called
    // swap::trampoline.
    //
    // There are 2 call frames in this setup, each containing the return address
    // followed by the %rbp value for that frame. This setup supports unwinding
    // using DWARF CFI as well as the frame pointer-based unwinding used by tools
    // such as perf or dtrace.
    let mut sp = StackPointer::new(stack.end());

    sp.push(0usize); // Padding to ensure the stack is properly aligned
    sp.push(fptr as usize); // Function that trampoline_2 should call

    // Call frame for trampoline_2. The CFA slot is updated by swap::trampoline
    // each time a context switch is performed.
    sp.push(trampoline_1 as usize + 2); // Return after the 2 nops
    sp.push(0xdeaddeaddead0cfa); // CFA slot

    // Call frame for swap::trampoline. We set up the %rbp value to point to the
    // parent call frame.
    let frame = sp.offset(0);
    sp.push(trampoline_2 as usize + 1); // Entry point, skip initial nop
    sp.push(frame as usize); // Pointer to parent call frame

    // save the sp in register
    regs.reg[0] = sp.offset(0) as usize;
}

#[repr(C)]
#[derive(Debug)]
pub struct Registers {
    // 0: rsp
    // 1~3: tib
    reg: [usize; 4],
}

impl Registers {
    pub fn new() -> Registers {
        Registers { reg: [0; 4] }
    }

    #[inline]
    pub fn prefetch(&self) {
        unsafe {
            prefetch_asm(self as *const _ as *const usize);
            prefetch_asm(self.reg[0] as *const usize);
        }
    }
}
