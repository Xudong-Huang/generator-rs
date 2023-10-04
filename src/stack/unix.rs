use std::io;
use std::mem;
use std::os::raw::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::usize;

use super::SysStack;

#[cfg(any(
    target_os = "openbsd",
    target_os = "macos",
    target_os = "ios",
    target_os = "android",
    target_os = "illumos",
    target_os = "solaris"
))]
const MAP_STACK: libc::c_int = 0;

#[cfg(not(any(
    target_os = "openbsd",
    target_os = "macos",
    target_os = "ios",
    target_os = "android",
    target_os = "illumos",
    target_os = "solaris"
)))]
const MAP_STACK: libc::c_int = libc::MAP_STACK;

pub unsafe fn allocate_stack(size: usize) -> io::Result<SysStack> {
    const NULL: *mut libc::c_void = 0 as *mut libc::c_void;
    const PROT: libc::c_int = libc::PROT_READ | libc::PROT_WRITE;
    const TYPE: libc::c_int = libc::MAP_PRIVATE | libc::MAP_ANON | MAP_STACK;

    let ptr = libc::mmap(NULL, size, PROT, TYPE, -1, 0);

    if ptr == libc::MAP_FAILED {
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

    debug_assert!(stack.len() % page_size == 0 && stack.len() != 0);

    let ret = {
        let bottom = stack.bottom();
        libc::mprotect(bottom, page_size, libc::PROT_NONE)
    };

    if ret != 0 {
        Err(io::Error::last_os_error())
    } else {
        let bottom = (stack.bottom() as usize + page_size) as *mut c_void;
        Ok(SysStack::new(stack.top(), bottom))
    }
}

pub unsafe fn deallocate_stack(ptr: *mut c_void, size: usize) {
    libc::munmap(ptr, size);
}

pub fn page_size() -> usize {
    static PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);

    let mut ret = PAGE_SIZE.load(Ordering::Relaxed);

    if ret == 0 {
        unsafe {
            ret = libc::sysconf(libc::_SC_PAGESIZE) as usize;
        }

        PAGE_SIZE.store(ret, Ordering::Relaxed);
    }

    ret
}

pub fn min_stack_size() -> usize {
    // Previously libc::SIGSTKSZ has been used for this, but it proofed to be very unreliable,
    // because the resulting values varied greatly between platforms.
    page_size()
}

#[cfg(not(target_os = "fuchsia"))]
pub fn max_stack_size() -> usize {
    static PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);

    let mut ret = PAGE_SIZE.load(Ordering::Relaxed);

    if ret == 0 {
        let mut limit = mem::MaybeUninit::uninit();
        let limitret = unsafe { libc::getrlimit(libc::RLIMIT_STACK, limit.as_mut_ptr()) };
        let limit = unsafe { limit.assume_init() };

        if limitret == 0 {
            ret = if limit.rlim_max == libc::RLIM_INFINITY
                || limit.rlim_max > (usize::MAX as libc::rlim_t)
            {
                usize::MAX
            } else {
                limit.rlim_max as usize
            };

            PAGE_SIZE.store(ret, Ordering::Relaxed);
        } else {
            ret = 1024 * 1024 * 1024;
        }
    }

    ret
}

#[cfg(target_os = "fuchsia")]
pub fn max_stack_size() -> usize {
    // Fuchsia doesn't have a platform defined hard cap.
    usize::MAX
}

pub mod overflow {
    use crate::rt::{guard, ContextStack};
    use crate::yield_::yield_now;
    use crate::Error;
    use libc::{sigaction, sighandler_t, SA_ONSTACK, SA_SIGINFO, SIGBUS, SIGSEGV, SIG_DFL};
    use std::mem;
    use std::sync::Once;

    static mut SIG_HANDLER: sighandler_t = 0;

    unsafe extern "C" fn signal_handler(
        signum: libc::c_int,
        info: *mut libc::siginfo_t,
        ctx: *mut libc::c_void,
    ) {
        let handler = SIG_HANDLER;
        if handler == SIG_DFL {
            return;
        }

        let addr = (*info).si_addr() as usize;
        let guard = guard::current();

        if !guard.contains(&addr) {
            let handler: unsafe extern "C" fn(
                signum: libc::c_int,
                info: *mut libc::siginfo_t,
                _data: *mut libc::c_void,
            ) = mem::transmute(handler);

            handler(signum, info, ctx);
            return;
        }

        eprintln!(
            "\ncoroutine in thread '{}' has overflowed its stack\n",
            std::thread::current().name().unwrap_or("<unknown>")
        );

        ContextStack::current().top().err = Some(Box::new(Error::StackErr));
        yield_now();

        std::process::abort();
    }

    unsafe fn init() {
        let mut action: sigaction = mem::zeroed();
        let mut old_action: sigaction = mem::zeroed();

        action.sa_flags = SA_SIGINFO | SA_ONSTACK;
        action.sa_sigaction = signal_handler as sighandler_t;

        for signal in [SIGSEGV, SIGBUS] {
            sigaction(signal, &action, &mut old_action);
            SIG_HANDLER = old_action.sa_sigaction;
        }
    }

    pub fn init_once() {
        static INIT_ONCE: Once = Once::new();

        INIT_ONCE.call_once(|| unsafe {
            init();
        })
    }
}
