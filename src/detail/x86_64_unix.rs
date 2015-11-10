use detail::{align_down, mut_offset};
use reg_context::InitFn;

#[repr(simd)]
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct u32x4(u32, u32, u32, u32);

impl u32x4 {
    pub fn new(a: u32, b: u32, c: u32, d: u32) -> u32x4 {
        u32x4(a, b, c, d)
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct Registers {
    gpr: [usize; 10],
    _xmm: [u32x4; 6],
}

impl Registers {
    pub fn new() -> Registers {
        Registers {
            gpr: [0; 10],
            _xmm: [u32x4::new(0, 0, 0, 0); 6],
        }
    }
}

pub fn initialize_call_frame(regs: &mut Registers,
                             fptr: InitFn,
                             arg: usize,
                             arg2: *mut usize,
                             sp: *mut usize) {

    #[inline(never)]
    unsafe fn bootstrap_green_task() {
        asm!("
            mov %r12, %rdi
            mov %r13, %rsi
            mov %r14, 8(%rsp)
        "
        :
        :
        : "{rdi}", "{rsi}", "memory"
        : "volatile");
    }

    // Redefinitions from rt/arch/x86_64/regs.h
    const RUSTRT_RSP: usize = 1;
    const RUSTRT_RBP: usize = 2;
    const RUSTRT_R12: usize = 4;
    const RUSTRT_R13: usize = 5;
    const RUSTRT_R14: usize = 6;

    let sp = align_down(sp);
    let sp = mut_offset(sp, -2);

    // The final return address. 0 indicates the bottom of the stack
    unsafe {
        *sp = 0;
    }

    debug!("creating call framenn");
    debug!("fptr {:#x}", fptr as usize);
    debug!("arg {:#x}", arg);
    debug!("sp {:?}", sp);

    // These registers are frobbed by rust_bootstrap_green_task into the right
    // location so we can invoke the "real init function", `fptr`.
    regs.gpr[RUSTRT_R12] = arg;
    regs.gpr[RUSTRT_R13] = arg2 as usize;
    regs.gpr[RUSTRT_R14] = fptr as usize;

    // These registers are picked up by the regular context switch paths. These
    // will put us in "mostly the right context" except for frobbing all the
    // arguments to the right place. We have the small trampoline code inside of
    // rust_bootstrap_green_task to do that.
    regs.gpr[RUSTRT_RSP] = mut_offset(sp, -2) as usize;

    // this is prepared for the swap context
    unsafe {
        *mut_offset(sp, -2) = bootstrap_green_task as usize;
        *mut_offset(sp, -1) = bootstrap_green_task as usize;
    }

    // Last base pointer on the stack should be 0
    regs.gpr[RUSTRT_RBP] = 0;
}

#[inline(never)]
pub unsafe fn swap_registers(out_regs: *mut Registers, in_regs: *const Registers) {
    // The first argument is in %rdi, and the second one is in %rsi

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

        movapd %xmm0, (10*8)(%rdi)
        movapd %xmm1, (12*8)(%rdi)
        movapd %xmm2, (14*8)(%rdi)
        movapd %xmm3, (16*8)(%rdi)
        movapd %xmm4, (18*8)(%rdi)
        movapd %xmm5, (20*8)(%rdi)


        mov (0*8)(%rsi), %rbx
        mov (1*8)(%rsi), %rsp
        mov (2*8)(%rsi), %rbp
        mov (4*8)(%rsi), %r12
        mov (5*8)(%rsi), %r13
        mov (6*8)(%rsi), %r14
        mov (7*8)(%rsi), %r15

        mov (3*8)(%rsi), %rdi

        movapd (10*8)(%rsi), %xmm0
        movapd (12*8)(%rsi), %xmm1
        movapd (14*8)(%rsi), %xmm2
        movapd (16*8)(%rsi), %xmm3
        movapd (18*8)(%rsi), %xmm4
        movapd (20*8)(%rsi), %xmm5
    "
    :
    : "{rdi}"(out_regs), "{rsi}"(in_regs)
    : "memory", "{rbx}", "{rsp}", "{rbp}", "{r12}", "{r13}", "{r14}", "{r15}",
      "{rdi}", "{xmm0}", "{xmm1}", "{xmm2}", "{xmm3}", "{xmm4}", "{xmm5}"
    : "volatile");
}
