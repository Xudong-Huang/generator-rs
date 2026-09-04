use crate::detail::{align_down, mut_offset};
use crate::stack::Stack;

std::arch::global_asm!(include_str!("asm/asm_s390x_sysv_elf.S"));

// first argument is task handle, second is thunk ptr
pub type InitFn = extern "C" fn(usize, *mut usize) -> !;

pub extern "C" fn gen_init(a1: usize, a2: *mut usize) -> ! {
    super::gen::gen_init_impl(a1, a2)
}

extern "C" {
    pub fn bootstrap_green_task();
    pub fn prefetch(data: *const usize);
    pub fn swap_registers(out_regs: *mut Registers, in_regs: *const Registers);
}

#[repr(C)]
#[derive(Debug)]
pub struct Registers {
    // We save the 10 callee-saved general purpose registers:
    //  0: r6
    //  1: r7
    //  2: r8
    //  3: r9
    //  4: r10
    //  5: r11
    //  6: r12
    //  7: r13
    //  8: r14 (return address)
    //  9: r15 (stack pointer)
    // and the 8 callee-saved floating point registers:
    // 10-17: f8-f15
    //
    // The order matches the register range of STMG/LMG, see
    // asm/asm_s390x_sysv_elf.S
    gpr: [usize; 32],
}

// The ELF ABI requires every caller to reserve a 160 byte register save area
// at the stack pointer, the callee stores r6-r15 and f0-f6 into it.
const LINKAGE_AREA: isize = (160 / std::mem::size_of::<usize>()) as isize;

impl Registers {
    pub fn new() -> Registers {
        Registers { gpr: [0; 32] }
    }

    #[inline]
    pub fn prefetch(&self) {
        let ptr = self.gpr[SP] as *const usize;
        unsafe {
            prefetch(ptr); // SP
            prefetch(ptr.add(8)); // SP + 64
        }
    }
}

const R7: usize = 1;
const R8: usize = 2;
const R9: usize = 3;
const RA: usize = 8; // r14
const SP: usize = 9; // r15

pub fn initialize_call_frame(
    regs: &mut Registers,
    fptr: InitFn,
    arg: usize,
    arg2: *mut usize,
    stack: &Stack,
) {
    // leave room for the register save area the generator entry point writes to
    let sp = mut_offset(align_down(stack.end()), -LINKAGE_AREA);

    // terminate the back chain
    unsafe { *sp = 0 };

    // These registers are frobbed by bootstrap_green_task into the right
    // location so we can invoke the "real init function", `fptr`.
    regs.gpr[R7] = arg;
    regs.gpr[R8] = arg2 as usize;
    regs.gpr[R9] = fptr as usize;

    regs.gpr[RA] = bootstrap_green_task as *const () as usize;

    // setup the init stack
    // this is prepared for the swap context
    regs.gpr[SP] = sp as usize;
}
