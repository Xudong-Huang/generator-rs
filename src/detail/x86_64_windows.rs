use detail::{align_down, mut_offset};
use reg_context::InitFn;

use super::Registers;

pub fn initialize_call_frame(regs: &mut Registers,
                             fptr: InitFn,
                             arg: usize,
                             arg2: *mut usize,
                             sp: *mut usize) {

    #[inline(never)]
    unsafe fn bootstrap_green_task() {
        asm!("
            mov %r12, %rcx     // setup the function arg
            mov %r13, %rdx     // setup the function arg
            mov %r14, 8(%rsp)  // this is the new return adrress
        "
        : // no output
        : // no input
        : "memory"
        : "volatile");
    }

    // Redefinitions from rt/arch/x86_64/regs.h
    const RUSTRT_RSP: usize = 1;
    const RUSTRT_RBP: usize = 2;
    const RUSTRT_R12: usize = 4;
    const RUSTRT_R13: usize = 5;
    const RUSTRT_R14: usize = 6;

    let sp = align_down(sp);

    // These registers are frobbed by rust_bootstrap_green_task into the right
    // location so we can invoke the "real init function", `fptr`.
    regs.gpr[RUSTRT_R12] = arg;
    regs.gpr[RUSTRT_R13] = arg2 as usize;
    regs.gpr[RUSTRT_R14] = fptr as usize;

    // These registers are picked up by the regular context switch paths. These
    // will put us in "mostly the right context" except for frobbing all the
    // arguments to the right place. We have the small trampoline code inside of
    // rust_bootstrap_green_task to do that.
    regs.gpr[RUSTRT_RSP] = mut_offset(sp, -8) as usize;

    // this is prepared for the swap context
    // different platform/debug has different offset between sp and ret
    unsafe {
        *mut_offset(sp, -8) = bootstrap_green_task as usize; // release
        *mut_offset(sp, -7) = bootstrap_green_task as usize; // release
        *mut_offset(sp, -6) = bootstrap_green_task as usize; // debug
        *mut_offset(sp, -5) = bootstrap_green_task as usize; // debug
        *mut_offset(sp, -4) = 0;
        *mut_offset(sp, -3) = 0;
        *mut_offset(sp, -2) = 0;
        *mut_offset(sp, -1) = 0;
    }

    // Last base pointer on the stack should be 0
    regs.gpr[RUSTRT_RBP] = 0;
}

#[inline(never)]
pub unsafe fn swap_registers(out_regs: *mut Registers, in_regs: *const Registers) {
    // The first argument is in %rcx, and the second one is in %rdx

    // Save registers
    asm!("
        mov %rbx, (0*8)(%rcx)
        mov %rsp, (1*8)(%rcx)
        mov %rbp, (2*8)(%rcx)
        mov %r12, (4*8)(%rcx)
        mov %r13, (5*8)(%rcx)
        mov %r14, (6*8)(%rcx)
        mov %r15, (7*8)(%rcx)

        //mov %rdi, (9*8)(%rcx)
        //mov %rsi, (10*8)(%rcx)

        mov %rcx, (3*8)(%rcx)


        mov (0*8)(%rdx), %rbx
        mov (1*8)(%rdx), %rsp
        mov (2*8)(%rdx), %rbp
        mov (4*8)(%rdx), %r12
        mov (5*8)(%rdx), %r13
        mov (6*8)(%rdx), %r14
        mov (7*8)(%rdx), %r15

        //mov (9*8)(%rdx), %rdi
        //mov (10*8)(%rdx), %rsi

        mov (3*8)(%rdx), %rcx
    "
    :
    : "{rcx}"(out_regs), "{rdx}"(in_regs)
    : "memory"
    : "volatile");
}
