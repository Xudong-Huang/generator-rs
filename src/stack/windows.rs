use std::io;
use std::mem;
use std::os::raw::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::usize;

use winapi::shared::basetsd::SIZE_T;
use winapi::shared::minwindef::{DWORD, LPVOID};
use winapi::um::memoryapi::{VirtualAlloc, VirtualFree, VirtualProtect};
use winapi::um::sysinfoapi::GetSystemInfo;
use winapi::um::winnt::{
    MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_GUARD, PAGE_READONLY, PAGE_READWRITE,
};

use super::SysStack;

pub unsafe fn allocate_stack(size: usize) -> io::Result<SysStack> {
    const NULL: LPVOID = 0 as LPVOID;
    const PROT: DWORD = PAGE_READWRITE;
    const TYPE: DWORD = MEM_COMMIT | MEM_RESERVE;

    let ptr = VirtualAlloc(NULL, size as SIZE_T, TYPE, PROT);

    if ptr == NULL {
        Err(io::Error::last_os_error())
    } else {
        Ok(SysStack::new(
            (ptr as usize + size) as *mut c_void,
            ptr as *mut c_void,
        ))
    }
}

pub unsafe fn protect_stack(stack: &SysStack) -> io::Result<SysStack> {
    const TYPE: DWORD = PAGE_READONLY | PAGE_GUARD;

    let page_size = page_size();
    let mut old_prot: DWORD = 0;

    debug_assert!(stack.len() % page_size == 0 && stack.len() != 0);

    let ret = {
        let page_size = page_size as SIZE_T;
        VirtualProtect(stack.bottom() as LPVOID, page_size, TYPE, &mut old_prot)
    };

    if ret == 0 {
        Err(io::Error::last_os_error())
    } else {
        let bottom = (stack.bottom() as usize + page_size) as *mut c_void;
        Ok(SysStack::new(stack.top(), bottom))
    }
}

pub unsafe fn deallocate_stack(ptr: *mut c_void, _: usize) {
    VirtualFree(ptr as LPVOID, 0, MEM_RELEASE);
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
