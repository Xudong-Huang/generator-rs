use std::mem;
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
    regs.reg[1] = stack.end() as usize;
    regs.reg[2] = stack.begin() as usize;
    regs.reg[3] = 0;
}

// load TIB context into the root regs
#[inline(always)]
unsafe fn load_context(regs: *mut Registers) {
    asm!(
    r#"
        /* load NT_TIB */
        movq  %gs:(0x30), %r10
        /* save current stack base */
        movq  0x08(%r10), %rax
        mov  %rax, (1*8)(%rcx)
        /* save current stack limit */
        movq  0x10(%r10), %rax
        mov  %rax, (2*8)(%rcx)
        /* save current deallocation stack */
        movq  0x1478(%r10), %rax
        mov  %rax, (3*8)(%rcx)
    "#
    :
    : "{rcx}" (regs)
    : "rax", "r10", "memory"
    : "volatile"
    );
}

// this is only used in windows platform which need to save the TIB info
#[inline(always)]
pub unsafe fn restore_context(regs: *mut Registers) {
    // load tib and save in the src
    // load dst and save to the tib
    asm!(
    r#"
        /* load NT_TIB */
        movq  %gs:(0x30), %r10
        /* restore deallocation stack */
        mov  (3*8)(%rcx), %rax
        movq  %rax, 0x1478(%r10)
        /* restore stack limit */
        mov  (2*8)(%rcx), %rax
        movq  %rax, 0x10(%r10)
        /* restore stack base */
        mov  (1*8)(%rcx), %rax
        movq  %rax, 0x8(%r10)
    "#
    :
    : "{rcx}" (regs)
    : "rax", "r10", "memory"
    : "volatile");
}

#[inline(always)]
pub unsafe fn swap_link(
    arg: usize,
    new_sp: StackPointer,
    new_stack_base: *mut usize,
) -> (usize, StackPointer) {
    let ret: usize;
    let ret_sp: usize;
    asm!(
    r#"
        # Push the return address
        leaq    0f(%rip), %rax
        pushq   %rax

        # Save frame pointer explicitly; the unwinder uses it to find CFA of
        # the caller, and so it has to have the correct value immediately after
        # the call instruction that invoked the trampoline.
        pushq   %rbp

        # Link the call stacks together by writing the current stack bottom
        # address to the CFA slot in the new stack.
        movq    %rsp, -32(%rdi)

        # Pass the stack pointer of the old context to the new one.
        movq    %rsp, %rdx

        # Load stack pointer of the new context.
        movq    %rsi, %rsp

        # Restore frame pointer of the new context.
        popq    %rbp

        # Return into the new context. Use `pop` and `jmp` instead of a `ret`
        # to avoid return address mispredictions (~8ns per `ret` on Ivy Bridge).
        popq    %rax
        jmpq    *%rax
      0:
    "#
    : "={rcx}" (ret)
      "={rdx}" (ret_sp)
    : "{rcx}" (arg)
      "{rsi}" (new_sp.offset(0))
      "{rdi}" (new_stack_base)
    : "rax",   "rbx",   /*"rcx",   "rdx",*/ "rsi",   "rdi",  /* "rbp",   "rsp",*/
      "r8",    "r9",    "r10",   "r11",   "r12",   "r13",   "r14",   "r15",
      "mm0",   "mm1",   "mm2",   "mm3",   "mm4",   "mm5",   "mm6",   "mm7",
      "xmm0",  "xmm1",  "xmm2",  "xmm3",  "xmm4",  "xmm5",  "xmm6",  "xmm7",
      "xmm8",  "xmm9",  "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15",
      "xmm16", "xmm17", "xmm18", "xmm19", "xmm20", "xmm21", "xmm22", "xmm23",
      "xmm24", "xmm25", "xmm26", "xmm27", "xmm28", "xmm29", "xmm30", "xmm31",
      "cc", "dirflag", "fpsr", "flags", "memory"
      // Ideally, we would set the LLVM "noredzone" attribute on this function
      // (and it would be propagated to the call site). Unfortunately, rustc
      // provides no such functionality. Fortunately, by a lucky coincidence,
      // the "alignstack" LLVM inline assembly option does exactly the same
      // thing on x86_64.
    : "volatile", "alignstack");
    (ret, mem::transmute(ret_sp))
}

#[inline(always)]
pub unsafe fn swap(arg: usize, new_sp: StackPointer) -> (usize, StackPointer) {
    // This is identical to swap_link, but without the write to the CFA slot.
    let ret: usize;
    let ret_sp: usize;
    asm!(
    r#"
        leaq    0f(%rip), %rax
        pushq   %rax
        pushq   %rbp
        movq    %rsp, %rdx
        movq    %rsi, %rsp
        popq    %rbp
        popq    %rax
        jmpq    *%rax
      0:
    "#
    : "={rcx}" (ret)
      "={rdx}" (ret_sp)
    : "{rcx}" (arg)
      "{rsi}" (new_sp.offset(0))
    : "rax",   "rbx",   /*"rcx",   "rdx",*/ "rsi",   "rdi",  /* "rbp",   "rsp",*/
      "r8",    "r9",    "r10",   "r11",   "r12",   "r13",   "r14",   "r15",
      "mm0",   "mm1",   "mm2",   "mm3",   "mm4",   "mm5",   "mm6",   "mm7",
      "xmm0",  "xmm1",  "xmm2",  "xmm3",  "xmm4",  "xmm5",  "xmm6",  "xmm7",
      "xmm8",  "xmm9",  "xmm10", "xmm11", "xmm12", "xmm13", "xmm14", "xmm15",
      "xmm16", "xmm17", "xmm18", "xmm19", "xmm20", "xmm21", "xmm22", "xmm23",
      "xmm24", "xmm25", "xmm26", "xmm27", "xmm28", "xmm29", "xmm30", "xmm31",
      "cc", "dirflag", "fpsr", "flags", "memory"
    : "volatile", "alignstack");
    (ret, mem::transmute(ret_sp))
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

    // use for root thread register init
    pub fn root() -> Registers {
        let mut regs = Self::new();
        unsafe { load_context(&mut regs) };
        regs
    }

    #[inline]
    pub fn get_sp(&self) -> StackPointer {
        unsafe { StackPointer::new(self.reg[0] as *mut usize) }
    }

    #[inline]
    pub fn set_sp(&mut self, sp: StackPointer) {
        self.reg[0] = unsafe { mem::transmute(sp) };
    }

    #[inline]
    pub fn prefetch(&self) {
        unsafe {
            prefetch_asm(self as *const _ as *const usize);
            prefetch_asm(self.reg[0] as *const usize);
        }
    }
}
