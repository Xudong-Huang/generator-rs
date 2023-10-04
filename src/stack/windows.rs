use std::io;
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::usize;

use windows::Win32::System::Memory::*;
use windows::Win32::System::SystemInformation::*;

use super::SysStack;

pub unsafe fn allocate_stack(size: usize) -> io::Result<SysStack> {
    let ptr = VirtualAlloc(
        Some(ptr::null()),
        size,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_READWRITE,
    );

    if ptr.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(SysStack::new(
            (ptr as usize + size) as *mut c_void,
            ptr as *mut c_void,
        ))
    }
}

pub unsafe fn protect_stack(stack: &SysStack) -> io::Result<SysStack> {
    let page_size = page_size();
    let mut old_prot = PAGE_PROTECTION_FLAGS(0);

    debug_assert!(stack.len() % page_size == 0 && stack.len() != 0);

    let ret = VirtualProtect(
        stack.bottom(),
        page_size,
        PAGE_READONLY | PAGE_GUARD,
        &mut old_prot,
    );

    if ret.is_err() {
        Err(io::Error::last_os_error())
    } else {
        let bottom = (stack.bottom() as usize + page_size) as *mut c_void;
        Ok(SysStack::new(stack.top(), bottom))
    }
}

pub unsafe fn deallocate_stack(ptr: *mut c_void, _: usize) {
    let _ = VirtualFree(ptr, 0, MEM_RELEASE);
}

pub fn page_size() -> usize {
    static PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);

    let mut ret = PAGE_SIZE.load(Ordering::Relaxed);

    if ret == 0 {
        ret = unsafe {
            let mut info = mem::zeroed();
            GetSystemInfo(&mut info);
            info.dwPageSize as usize
        };

        PAGE_SIZE.store(ret, Ordering::Relaxed);
    }

    ret
}

// Windows does not seem to provide a stack limit API
pub fn min_stack_size() -> usize {
    page_size()
}

// Windows does not seem to provide a stack limit API
pub fn max_stack_size() -> usize {
    usize::MAX
}

pub mod overflow {
    use crate::rt::guard;
    //use crate::Error;
    use std::sync::Once;
    use windows::Win32::Foundation::EXCEPTION_STACK_OVERFLOW;
    use windows::Win32::System::Diagnostics::Debug::{
        AddVectoredExceptionHandler, EXCEPTION_POINTERS,
    };

    unsafe extern "system" fn vectored_handler(exception_info: *mut EXCEPTION_POINTERS) -> i32 {
        const EXCEPTION_CONTINUE_SEARCH: i32 = 0x0;
        //const EXCEPTION_CONTINUE_EXECUTION: i32 = 0xffffffffu32 as i32;

        let rec = &(*(*exception_info).ExceptionRecord);
        //let context = &mut (*(*exception_info).ContextRecord);

        if rec.ExceptionCode == EXCEPTION_STACK_OVERFLOW
            && guard::current().contains(&(rec.ExceptionAddress as usize))
        {
            eprintln!(
                "\ncoroutine in thread '{}' has overflowed its stack\n",
                std::thread::current().name().unwrap_or("<unknown>")
            );
            /*
                       let env = ContextStack::current();
                       let cur = env.top();
                       cur.err = Some(Box::new(Error::StackErr));

                       let parent = env.pop_context(cur as *mut _);
                       let &[rbx, rsp, rbp, _, r12, r13, r14, r15, _, _, _, stack_base, stack_limit, dealloc_stack, ..] =
                           &parent.regs.regs.gpr;

                       let rip = *(rsp as *const usize);
                       let rsp = rsp + std::mem::size_of::<usize>();

                       context.Rbx = rbx as u64;
                       context.Rsp = rsp as u64;
                       context.Rbp = rbp as u64;
                       context.R12 = r12 as u64;
                       context.R13 = r13 as u64;
                       context.R14 = r14 as u64;
                       context.R15 = r15 as u64;
                       context.Rip = rip as u64;

                       let gs = context.SegGs as usize;
                       let teb = gs + 0x30;

                       *((teb + 0x08) as *mut usize) = stack_base;
                       *((teb + 0x10) as *mut usize) = stack_limit;
                       *((teb + 0x1478) as *mut usize) = dealloc_stack;

                       //yield_now();

            */

            //EXCEPTION_CONTINUE_EXECUTION

            std::process::abort();
        } else {
            EXCEPTION_CONTINUE_SEARCH
        }
    }

    unsafe fn init() {
        AddVectoredExceptionHandler(1, Some(vectored_handler));
    }

    pub fn init_once() {
        static INIT_ONCE: Once = Once::new();

        INIT_ONCE.call_once(|| unsafe {
            init();
        })
    }
}
