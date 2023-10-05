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
    use libc::{
        sigaction, sigaddset, sigemptyset, sighandler_t, sigprocmask, sigset_t, SA_ONSTACK,
        SA_SIGINFO, SIGBUS, SIGSEGV,
    };
    use std::mem;
    use std::mem::MaybeUninit;
    use std::ptr::null_mut;
    use std::sync::Once;

    static mut SIG_ACTION: MaybeUninit<sigaction> = MaybeUninit::uninit();

    // Signal handler for the SIGSEGV and SIGBUS handlers. We've got guard pages
    // (unmapped pages) at the end of every thread's stack, so if a thread ends
    // up running into the guard page it'll trigger this handler. We want to
    // detect these cases and print out a helpful error saying that the stack
    // has overflowed. All other signals, however, should go back to what they
    // were originally supposed to do.
    //
    // If this is not a stack overflow, the handler un-registers itself and
    // then returns (to allow the original signal to be delivered again).
    // Returning from this kind of signal handler is technically not defined
    // to work when reading the POSIX spec strictly, but in practice it turns
    // out many large systems and all implementations allow returning from a
    // signal handler to work. For a more detailed explanation see the
    // comments on https://github.com/rust-lang/rust/issues/26458.
    unsafe extern "C" fn signal_handler(
        signum: libc::c_int,
        info: *mut libc::siginfo_t,
        _ctx: *mut libc::ucontext_t,
    ) {
        let addr = (*info).si_addr() as usize;
        let guard = guard::current();

        // we are unable to handle this
        if !guard.contains(&addr) {
            // SIG_ACTION is available after we registered our handler
            sigaction(signum, SIG_ACTION.assume_init_ref(), null_mut());

            return;
        }

        eprintln!(
            "\ncoroutine in thread '{}' has overflowed its stack\n",
            std::thread::current().name().unwrap_or("<unknown>")
        );

        ContextStack::current().top().err = Some(Box::new(Error::StackErr));

        let mut sigset: sigset_t = mem::zeroed();
        sigemptyset(&mut sigset);
        sigaddset(&mut sigset, signum);
        sigprocmask(libc::SIG_UNBLOCK, &sigset, null_mut());

        yield_now();

        // should never come back.
        std::process::abort();
    }

    #[cold]
    unsafe fn init() {
        let mut action: sigaction = mem::zeroed();

        action.sa_flags = SA_SIGINFO | SA_ONSTACK;
        action.sa_sigaction = signal_handler as sighandler_t;

        for signal in [SIGSEGV, SIGBUS] {
            sigaction(signal, &action, SIG_ACTION.as_mut_ptr());
        }
    }

    pub fn init_once() {
        static INIT_ONCE: Once = Once::new();

        INIT_ONCE.call_once(|| unsafe {
            init();
        })
    }
}
