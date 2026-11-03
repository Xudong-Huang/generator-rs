use crate::detail::{align_down, mut_offset};
use crate::stack::Stack;

// first argument is task handle, second is thunk ptr
pub type InitFn = extern "sysv64" fn(usize, *mut usize) -> !;

pub extern "sysv64" fn gen_init(a1: usize, a2: *mut usize) -> ! {
    super::gen::gen_init_impl(a1, a2)
}

std::arch::global_asm!(include_str!("asm/asm_x86_64_sysv_pe.S"));

// #[cfg(not(nightly))]
//#[link(name = "asm", kind = "static")]
extern "sysv64" {
    pub fn bootstrap_green_task();
    pub fn prefetch_asm(data: *const usize);
    pub fn swap_registers(out_regs: *mut Registers, in_regs: *const Registers);
}

#[inline]
pub fn prefetch(data: *const usize) {
    unsafe { prefetch_asm(data) }
}

// windows need to restore xmm6~xmm15, for most cases only use two xmm registers
// so we use sysv64
#[repr(C)]
#[derive(Debug)]
pub struct Registers {
    pub(crate) gpr: [usize; 16],
}

impl Registers {
    pub fn new() -> Registers {
        Registers { gpr: [0; 16] }
    }

    #[inline]
    pub fn prefetch(&self) {
        let ptr = self.gpr[1] as *const usize;
        unsafe {
            prefetch(ptr); // RSP
            prefetch(ptr.add(8)); // RSP + 8
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
    const RUSTRT_STACK_BASE: usize = 11;
    const RUSTRT_STACK_LIMIT: usize = 12;
    const RUSTRT_STACK_DEALLOC: usize = 13;

    let sp = align_down(stack.end());

    // These registers are frobbed by bootstrap_green_task into the right
    // location so we can invoke the "real init function", `fptr`.
    regs.gpr[RUSTRT_R12] = arg;
    regs.gpr[RUSTRT_R13] = arg2 as usize;
    regs.gpr[RUSTRT_R14] = fptr as usize;

    // Last base pointer on the stack should be 0
    regs.gpr[RUSTRT_RBP] = 0;

    regs.gpr[RUSTRT_STACK_BASE] = stack.end() as usize;
    regs.gpr[RUSTRT_STACK_LIMIT] = stack.begin() as usize;
    regs.gpr[RUSTRT_STACK_DEALLOC] = 0; //mut_offset(sp, -8192) as usize;

    // setup the init stack
    // this is prepared for the swap context
    regs.gpr[RUSTRT_RSP] = mut_offset(sp, -2) as usize;

    unsafe {
        // leave enough space for RET
        *mut_offset(sp, -2) = bootstrap_green_task as *const () as usize;
        *mut_offset(sp, -1) = 0;
    }
}
