use crate::detail::align_down;
use crate::stack::Stack;

// first argument is task handle, second is thunk ptr
pub type InitFn = extern "C" fn(usize, *mut usize) -> !;

pub extern "C" fn gen_init(a1: usize, a2: *mut usize) -> ! {
    super::gen::gen_init_impl(a1, a2);
}

extern "C" {
    pub fn bootstrap_green_task();
    pub fn prefetch(data: *const usize);
    pub fn swap_registers(out_regs: *mut Registers, in_regs: *const Registers);
}

#[repr(C)]
#[derive(Debug)]
pub struct Registers {
    gpr: [usize; 32],
    // array containing all registers. in order:
    // lr
    // cr
    // fp
    // toc (r2)
    // thread pointer (r13)
    // r14-r31
    // TODO: vector registers
    // we use r14 and r15 to store the parameters when initialising a call frame.
    // similar to the x86_64 implementation
}

// register indices:
const REG_LR: usize = 0;
// const REG_CR: usize = 1;
const REG_FP: usize = 2;
// const REG_TOC: usize = 3;
// const REG_THREAD_POINTER: usize = 4;
const REG_R14: usize = 5;
const REG_R15: usize = 6;
const REG_R16: usize = 7;

// TODO: consider Thread local storage (TLS) ABI.

impl Registers {
    pub fn new() -> Self {
        Self { gpr: [0; 32] }
    }

    pub fn prefetch(&self) {
        unsafe {
            prefetch(&self.gpr[0]);
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
    let sp = align_down(stack.end());

    regs.gpr[REG_FP] = sp as usize;
    regs.gpr[REG_R14] = arg;
    regs.gpr[REG_R15] = arg2 as usize;
    regs.gpr[REG_R16] = fptr as usize;

    regs.gpr[REG_LR] = bootstrap_green_task as usize;
}

#[test]
fn test_debug() {
    let mut test = Registers::new();
    // println!("before swap call!");

    unsafe { swap_registers(&mut test, &mut test) }

    println!("{:?}", test.gpr);
}
