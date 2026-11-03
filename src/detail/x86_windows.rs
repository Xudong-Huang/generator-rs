use crate::detail::{align_down, mut_offset};
use crate::stack::Stack;

// first argument is task handle, second is thunk ptr
pub type InitFn = extern "C" fn(usize, *mut usize) -> !;

pub extern "C" fn gen_init(a1: usize, a2: *mut usize) -> ! {
    super::gen::gen_init_impl(a1, a2)
}

std::arch::global_asm!(include_str!("asm/asm_x86_pe.S"));

extern "C" {
    pub fn bootstrap_green_task();
    pub fn prefetch_asm(data: *const usize);
    pub fn swap_registers(out_regs: *mut Registers, in_regs: *const Registers);
}

#[inline]
pub fn prefetch(data: *const usize) {
    unsafe { prefetch_asm(data) }
}

#[repr(C)]
#[derive(Debug)]
pub struct Registers {
    // The order matches the loads and stores in asm/asm_x86_pe.S
    //  0: ebx
    //  1: esp
    //  2: ebp
    //  3: esi
    //  4: edi
    //  5: SEH chain (fs:[0x00])
    //  6: stack base (fs:[0x04])
    //  7: stack limit (fs:[0x08])
    //  8: deallocation stack (fs:[0x0E0C])
    pub(crate) gpr: [usize; 9],
}

impl Registers {
    pub fn new() -> Registers {
        Registers { gpr: [0; 9] }
    }

    #[inline]
    pub fn prefetch(&self) {
        let ptr = self.gpr[ESP] as *const usize;
        prefetch(ptr); // ESP
        prefetch(unsafe { ptr.add(16) }); // ESP + 64
    }
}

const EBX: usize = 0;
const ESP: usize = 1;
const EBP: usize = 2;
const ESI: usize = 3;
const EDI: usize = 4;
const SEH_CHAIN: usize = 5;
const STACK_BASE: usize = 6;
const STACK_LIMIT: usize = 7;
const STACK_DEALLOC: usize = 8;

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

    // the SEH records live on the stack, so the generator starts with its own
    // empty chain, which is terminated by -1
    regs.gpr[SEH_CHAIN] = usize::MAX;
    regs.gpr[STACK_BASE] = stack.end() as usize;
    regs.gpr[STACK_LIMIT] = stack.begin() as usize;
    regs.gpr[STACK_DEALLOC] = 0;

    // setup the init stack
    // this is prepared for the swap context
    regs.gpr[ESP] = mut_offset(sp, -2) as usize;

    unsafe {
        // leave enough space for RET
        *mut_offset(sp, -2) = bootstrap_green_task as *const () as usize;
        *mut_offset(sp, -1) = 0;
    }
}
