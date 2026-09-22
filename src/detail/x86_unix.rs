use crate::detail::{align_down, mut_offset};
use crate::stack::Stack;

// first argument is task handle, second is thunk ptr
pub type InitFn = extern "C" fn(usize, *mut usize) -> !;

pub extern "C" fn gen_init(a1: usize, a2: *mut usize) -> ! {
    super::gen::gen_init_impl(a1, a2)
}

std::arch::global_asm!(include_str!("asm/asm_x86_sysv_elf.S"));

extern "C" {
    pub fn bootstrap_green_task();
    pub fn prefetch(data: *const usize);
    pub fn swap_registers(out_regs: *mut Registers, in_regs: *const Registers);
}

#[repr(C)]
#[derive(Debug)]
pub struct Registers {
    // We save the 5 callee-saved registers:
    //  0: ebx
    //  1: esp
    //  2: ebp
    //  3: esi
    //  4: edi
    //
    // The order matches the loads and stores in asm/asm_x86_sysv_elf.S
    gpr: [usize; 8],
}

impl Registers {
    pub fn new() -> Registers {
        Registers { gpr: [0; 8] }
    }

    #[inline]
    pub fn prefetch(&self) {
        let ptr = self.gpr[ESP] as *const usize;
        unsafe {
            prefetch(ptr); // ESP
            prefetch(ptr.add(16)); // ESP + 64
        }
    }
}

const EBX: usize = 0;
const ESP: usize = 1;
const EBP: usize = 2;
const ESI: usize = 3;
const EDI: usize = 4;

pub fn initialize_call_frame(
    regs: &mut Registers,
    fptr: InitFn,
    arg: usize,
    arg2: *mut usize,
    stack: &Stack,
) {
    let sp = align_down(stack.end());

    // These registers are frobbed by bootstrap_green_task into the right
    // location so we can invoke the "real init function", `fptr`.
    regs.gpr[EBX] = arg;
    regs.gpr[ESI] = arg2 as usize;
    regs.gpr[EDI] = fptr as usize;

    // Last base pointer on the stack should be 0
    regs.gpr[EBP] = 0;

    // setup the init stack
    // this is prepared for the swap context
    regs.gpr[ESP] = mut_offset(sp, -2) as usize;

    unsafe {
        // leave enough space for RET
        *mut_offset(sp, -2) = bootstrap_green_task as *const () as usize;
        *mut_offset(sp, -1) = 0;
    }
}
