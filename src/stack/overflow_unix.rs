use crate::rt::{guard, ContextStack};

use crate::yield_::yield_now;
use libc::{sigaction, sighandler_t, SA_ONSTACK, SA_SIGINFO, SIGBUS, SIGSEGV};
use std::cell::UnsafeCell;
use std::mem::{self, MaybeUninit};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

struct SavedAction {
    ready: AtomicBool,
    action: UnsafeCell<MaybeUninit<sigaction>>,
}

// SAFETY: `action` is written at most once during `init()` before `ready` is
// stored with `Ordering::Release`, and only read after `ready.load(Ordering::Acquire)`
// observes `true`.
unsafe impl Sync for SavedAction {}

impl SavedAction {
    const fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
            action: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

static OLD_SIGSEGV: SavedAction = SavedAction::new();
static OLD_SIGBUS: SavedAction = SavedAction::new();

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
//
// A pointer to the exception context is passed as the third argument. This
// context is usually compatible with libc::ucontext_t. However some architectures
// (like powerpc64) do not provide the ucontext_t type in glibc. Since we do not
// use the context information, it is represented as a generic pointer.
// see: https://github.com/rust-lang/libc/pull/3986 / https://github.com/rust-lang/libc/pull/3986
unsafe extern "C" fn signal_handler(
    signum: libc::c_int,
    info: *mut libc::siginfo_t,
    ctx: *mut libc::c_void, // workaroung for ppc64le missing ucontext_t in rust libc. See: https://github.com/rust-lang/libc/issues/3964
) {
    let _ctx = &mut *ctx;
    let addr = (*info).si_addr() as usize;
    let stack_guard = guard::current();

    if !stack_guard.contains(&addr) {
        println!("{}", std::backtrace::Backtrace::force_capture());
        // Restore the prior disposition recorded before this handler was installed.
        let saved = match signum {
            SIGSEGV => Some(&OLD_SIGSEGV),
            SIGBUS => Some(&OLD_SIGBUS),
            _ => None,
        };
        if let Some(saved) = saved {
            if saved.ready.load(Ordering::Acquire) {
                sigaction(signum, (*saved.action.get()).as_ptr(), null_mut());
            }
        }

        // we are unable to handle this
        return;
    }

    // eprintln! is not async-signal-safe and crashes inside the handler on
    // android x86_64, so write the message directly
    let msg = b"\ncoroutine has overflowed its stack\n";
    libc::write(
        libc::STDERR_FILENO,
        msg.as_ptr() as *const libc::c_void,
        msg.len(),
    );

    let context = ContextStack::current().top();
    context.err = Some(Box::new(crate::Error::StackErr));
    context.overflowed = true;

    let mut sigset: libc::sigset_t = mem::zeroed();
    libc::sigemptyset(&mut sigset);
    libc::sigaddset(&mut sigset, signum);
    libc::sigprocmask(libc::SIG_UNBLOCK, &sigset, null_mut());

    yield_now();

    std::process::abort();
}

#[cold]
unsafe fn install_signal_handler(saved: &SavedAction, signal: libc::c_int, action: &sigaction) {
    // Query and publish the previous disposition before installing our handler,
    // keeping `sigaction` in `MaybeUninit` so partially initialized C fields
    // (such as unused words in `sigset_t`) are never materialized as a Rust value.
    // Only install our handler if querying succeeded so `signal_handler` always
    // has a saved disposition to restore. Callers modifying process-wide signal
    // dispositions must serialize those changes with generator initialization.
    if sigaction(signal, null(), (*saved.action.get()).as_mut_ptr()) == 0 {
        saved.ready.store(true, Ordering::Release);
        let _ = sigaction(signal, action, null_mut());
    }
}

#[cold]
unsafe fn init() {
    let mut action: sigaction = mem::zeroed();

    action.sa_flags = SA_SIGINFO | SA_ONSTACK;
    action.sa_sigaction = signal_handler as *const () as sighandler_t;

    for (saved, signal) in [(&OLD_SIGSEGV, SIGSEGV), (&OLD_SIGBUS, SIGBUS)] {
        install_signal_handler(saved, signal, &action);
    }
}

pub fn init_once() {
    static INIT_ONCE: Once = Once::new();

    INIT_ONCE.call_once(|| unsafe {
        init();
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RestoreSignal {
        signal: libc::c_int,
        prev: MaybeUninit<sigaction>,
    }

    impl RestoreSignal {
        unsafe fn capture(signal: libc::c_int) -> Self {
            let mut prev = MaybeUninit::uninit();
            assert_eq!(sigaction(signal, null(), prev.as_mut_ptr()), 0);
            Self { signal, prev }
        }
    }

    impl Drop for RestoreSignal {
        fn drop(&mut self) {
            unsafe {
                let _ = sigaction(self.signal, self.prev.as_ptr(), null_mut());
            }
        }
    }

    #[test]
    fn preserves_distinct_saved_dispositions() {
        assert_ne!(libc::SIG_DFL, libc::SIG_IGN);
        unsafe {
            let _restore_usr1 = RestoreSignal::capture(libc::SIGUSR1);
            let _restore_usr2 = RestoreSignal::capture(libc::SIGUSR2);

            let mut dfl_act: sigaction = mem::zeroed();
            dfl_act.sa_sigaction = libc::SIG_DFL;
            assert_eq!(sigaction(libc::SIGUSR1, &dfl_act, null_mut()), 0);

            let mut ign_act: sigaction = mem::zeroed();
            ign_act.sa_sigaction = libc::SIG_IGN;
            assert_eq!(sigaction(libc::SIGUSR2, &ign_act, null_mut()), 0);

            let mut replacement: sigaction = mem::zeroed();
            replacement.sa_flags = SA_SIGINFO | SA_ONSTACK;
            replacement.sa_sigaction = signal_handler as *const () as sighandler_t;

            let saved_first = SavedAction::new();
            let saved_second = SavedAction::new();
            let saved_invalid = SavedAction::new();

            install_signal_handler(&saved_first, libc::SIGUSR1, &replacement);
            install_signal_handler(&saved_second, libc::SIGUSR2, &replacement);
            install_signal_handler(&saved_invalid, -1, &replacement);

            assert!(saved_first.ready.load(Ordering::Acquire));
            assert!(saved_second.ready.load(Ordering::Acquire));
            assert!(!saved_invalid.ready.load(Ordering::Acquire));
            assert_eq!(
                (*(*saved_first.action.get()).as_ptr()).sa_sigaction,
                libc::SIG_DFL
            );
            assert_eq!(
                (*(*saved_second.action.get()).as_ptr()).sa_sigaction,
                libc::SIG_IGN
            );
        }
    }

    #[test]
    fn init_once_saves_sigsegv_and_sigbus_dispositions() {
        init_once();
        assert!(OLD_SIGSEGV.ready.load(Ordering::Acquire));
        assert!(OLD_SIGBUS.ready.load(Ordering::Acquire));
    }
}
