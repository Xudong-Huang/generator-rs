use detail::{align_down, mut_offset};
use reg_context::InitFn;
use stack::Stack;

#[cfg(not(nightly))]
#[link(name = "asm", kind = "static")]
extern "C" {
    pub fn bootstrap_green_task();
    pub fn prefetch(data: *const usize);
    pub fn swap_registers(out_regs: *mut Registers, in_regs: *const Registers);
}

#[cfg(nightly)]
mod asm {
    use super::Registers;
    /// prefetch data
    #[inline]
    pub unsafe extern "C" fn prefetch(data: *const usize) {
        asm!("
        prefetcht1 $0
        "
        : // no output
        : "m"(*data)
        :
        : "volatile");
    }
    #[inline(never)]
    #[naked]
    pub unsafe extern "C" fn bootstrap_green_task() {
        asm!("
        mov %r12, %rdi     // setup the function arg
        mov %r13, %rsi     // setup the function arg
        mov %r14, 8(%rsp)  // this is the new return adrress
        "
        : // no output
        : // no input
        : "memory"
        : "volatile");
    }

    #[inline(never)]
    pub unsafe extern "C" fn swap_registers(out_regs: *mut Registers, in_regs: *const Registers) {
        // The first argument is in %rdi, and the second one is in %rsi
        asm!(
            ""
            :
            : "{rdi}"(out_regs), "{rsi}"(in_regs)
            :
            :
        );

        // introduce this function to workaround rustc bug! (#6)
        // the naked function is not correct any more
        #[naked]
        unsafe extern "C" fn _swap_reg() {
            // Save registers
            asm!("
        mov %rbx, (0*8)(%rdi)
        mov %rsp, (1*8)(%rdi)
        mov %rbp, (2*8)(%rdi)
        mov %r12, (4*8)(%rdi)
        mov %r13, (5*8)(%rdi)
        mov %r14, (6*8)(%rdi)
        mov %r15, (7*8)(%rdi)

        mov %rdi, (3*8)(%rdi)

        mov (0*8)(%rsi), %rbx
        mov (1*8)(%rsi), %rsp
        mov (2*8)(%rsi), %rbp
        mov (4*8)(%rsi), %r12
        mov (5*8)(%rsi), %r13
        mov (6*8)(%rsi), %r14
        mov (7*8)(%rsi), %r15

        mov (3*8)(%rsi), %rdi
        "
        :
        : //"{rdi}"(out_regs), "{rsi}"(in_regs)
        : "memory"
        : "volatile");
        }
        _swap_reg()
    }
}
#[cfg(nightly)]
pub use self::asm::*;

#[repr(C)]
#[derive(Debug)]
pub struct Registers {
    gpr: [usize; 8],
}

impl Registers {
    pub fn new() -> Registers {
        Registers { gpr: [0; 8] }
    }

    #[inline]
    pub fn prefetch(&self) {
        unsafe {
            prefetch(self as *const _ as *const usize);
            prefetch(self.gpr[1] as *const usize);
        }
    }
}

pub fn initialize_call_frame(
    regs: &mut Registers,
    fptr: InitFn,
    arg: usize,
    arg2: *mut usize,
    stack: &Stack,
) {
    // Redefinitions from rt/arch/x86_64/regs.h
    const RUSTRT_RSP: usize = 1;
    const RUSTRT_RBP: usize = 2;
    const RUSTRT_R12: usize = 4;
    const RUSTRT_R13: usize = 5;
    const RUSTRT_R14: usize = 6;

    let sp = align_down(stack.end());

    // These registers are frobbed by rust_bootstrap_green_task into the right
    // location so we can invoke the "real init function", `fptr`.
    regs.gpr[RUSTRT_R12] = arg;
    regs.gpr[RUSTRT_R13] = arg2 as usize;
    regs.gpr[RUSTRT_R14] = fptr as usize;

    // These registers are picked up by the regular context switch paths. These
    // will put us in "mostly the right context" except for frobbing all the
    // arguments to the right place. We have the small trampoline code inside of
    // rust_bootstrap_green_task to do that.
    regs.gpr[RUSTRT_RSP] = mut_offset(sp, -4) as usize;

    // setup the init stack
    // this is prepared for the swap context
    // different platform/debug has different offset between sp and ret
    unsafe {
        *mut_offset(sp, -4) = bootstrap_green_task as usize;
        *mut_offset(sp, -3) = bootstrap_green_task as usize;
        // leave enough space for RET
        *mut_offset(sp, -2) = 0;
        *mut_offset(sp, -1) = 0;
    }

    // Last base pointer on the stack should be 0
    regs.gpr[RUSTRT_RBP] = 0;
}
