use crate::detail::{align_down, mut_offset};
use crate::stack::Stack;

// first argument is task handle, second is thunk ptr
pub type InitFn = extern "sysv64" fn(usize, *mut usize) -> !;

pub extern "sysv64" fn gen_init(a1: usize, a2: *mut usize) -> ! {
    super::gen::gen_init_impl(a1, a2)
}

cfg_select! {
    target_os = "macos" => {
        std::arch::global_asm!(include_str!("asm/asm_x86_64_sysv_macho.S"));
    }
    _ => {
        std::arch::global_asm!(include_str!("asm/asm_x86_64_sysv_elf.S"));
    }
}

extern "sysv64" {
    pub fn bootstrap_green_task();
    pub fn prefetch(data: *const usize);
    pub fn swap_registers(out_regs: *mut Registers, in_regs: *const Registers);
}

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

    let sp = align_down(stack.end());

    // These registers are frobbed by bootstrap_green_task into the right
    // location so we can invoke the "real init function", `fptr`.
    regs.gpr[RUSTRT_R12] = arg;
    regs.gpr[RUSTRT_R13] = arg2 as usize;
    regs.gpr[RUSTRT_R14] = fptr as usize;

    // Last base pointer on the stack should be 0
    regs.gpr[RUSTRT_RBP] = 0;

    // setup the init stack
    // this is prepared for the swap context
    regs.gpr[RUSTRT_RSP] = mut_offset(sp, -2) as usize;

    unsafe {
        // leave enough space for RET
        *mut_offset(sp, -2) = bootstrap_green_task as *const () as usize;
        *mut_offset(sp, -1) = 0;
    }
}
