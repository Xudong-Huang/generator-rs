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
unsafe fn init() {
    let mut action: sigaction = mem::zeroed();

    action.sa_flags = SA_SIGINFO | SA_ONSTACK;
    action.sa_sigaction = signal_handler as *const () as sighandler_t;

    for (saved, signal) in [(&OLD_SIGSEGV, SIGSEGV), (&OLD_SIGBUS, SIGBUS)] {
        // Query and publish the previous disposition before installing our handler,
        // keeping `sigaction` in `MaybeUninit` so partially initialized C fields
        // (such as unused words in `sigset_t`) are never materialized as a Rust value.
        // Only install our handler if querying succeeded so `signal_handler` always
        // has a saved disposition to restore. Callers modifying process-wide signal
        // dispositions must serialize those changes with generator initialization.
        if sigaction(signal, null(), (*saved.action.get()).as_mut_ptr()) == 0 {
            saved.ready.store(true, Ordering::Release);
            let _ = sigaction(signal, &action, null_mut());
        }
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

    #[test]
    fn preserves_distinct_saved_dispositions_in_child() {
        const ENV_KEY: &str = "GENERATOR_TEST_DISTINCT_SIGNAL_CHILD";
        if std::env::var_os(ENV_KEY).is_some() {
            assert_ne!(libc::SIG_DFL, libc::SIG_IGN);
            unsafe {
                let mut segv_act: sigaction = mem::zeroed();
                segv_act.sa_sigaction = libc::SIG_DFL;
                assert_eq!(sigaction(SIGSEGV, &segv_act, null_mut()), 0);

                let mut bus_act: sigaction = mem::zeroed();
                bus_act.sa_sigaction = libc::SIG_IGN;
                assert_eq!(sigaction(SIGBUS, &bus_act, null_mut()), 0);

                init_once();

                assert!(OLD_SIGSEGV.ready.load(Ordering::Acquire));
                assert!(OLD_SIGBUS.ready.load(Ordering::Acquire));
                let saved_segv = (*OLD_SIGSEGV.action.get()).as_ptr();
                let saved_bus = (*OLD_SIGBUS.action.get()).as_ptr();
                assert_eq!((*saved_segv).sa_sigaction, libc::SIG_DFL);
                assert_eq!((*saved_bus).sa_sigaction, libc::SIG_IGN);
            }
            return;
        }

        let exe = std::env::current_exe().unwrap();
        let output = std::process::Command::new(exe)
            .arg("--exact")
            .arg("stack::sys::overflow::tests::preserves_distinct_saved_dispositions_in_child")
            .arg("--nocapture")
            .env(ENV_KEY, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child test exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("1 passed"),
            "expected child test to execute, got: {stdout}"
        );
    }
}
