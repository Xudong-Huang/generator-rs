use std::io;
use std::mem;
use std::os::raw::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    clippy::all
)]
mod windows_bindings;

use windows_bindings::Windows::Win32::{GetSystemInfo, VirtualAlloc, VirtualFree, VirtualProtect};
use windows_sys::Win32::System::Memory::{
    MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_GUARD, PAGE_READONLY, PAGE_READWRITE,
};

use super::SysStack;

#[path = "overflow_windows.rs"]
pub mod overflow;

pub unsafe fn allocate_stack(size: usize) -> io::Result<SysStack> {
    let ptr = VirtualAlloc(None, size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);

    if ptr.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(SysStack::new(ptr.cast::<u8>().add(size).cast(), ptr))
    }
}

pub unsafe fn protect_stack(stack: &SysStack) -> io::Result<SysStack> {
    let page_size = page_size();
    let mut old_prot = mem::zeroed();

    debug_assert!(stack.len().is_multiple_of(page_size) && stack.len() != 0);

    if !VirtualProtect(
        stack.bottom(),
        page_size,
        PAGE_READONLY | PAGE_GUARD,
        &mut old_prot,
    )
    .as_bool()
    {
        return Err(io::Error::last_os_error());
    }

    let bottom = stack.bottom().cast::<u8>().add(page_size).cast::<c_void>();
    Ok(SysStack::new(stack.top(), bottom))
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
