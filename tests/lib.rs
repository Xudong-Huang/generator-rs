#![allow(deprecated)]
#![allow(unused_assignments)]

extern crate generator;

use generator::*;

// Exhausts the current stack without relying on tail-call optimization, so
// release builds cannot turn it into a loop.
#[cfg(all(not(target_os = "fuchsia"), not(windows)))]
#[inline(never)]
fn overflow_stack(n: usize) -> usize {
    let buf = [0u8; 512];
    std::hint::black_box(&buf);
    if n == usize::MAX {
        return 0;
    }
    let r = overflow_stack(n + 1);
    std::hint::black_box(r.wrapping_add(buf[0] as usize))
}

// Panic messages are formatted, and backtraced when `RUST_BACKTRACE` is set,
// on the generator's own stack. Cancelling or dropping a suspended generator
// also panics on that stack to unwind the body. Panic tests need room for
// that: on a minimal stack the panic path overflows, and the overflow handler
// yields out without unwinding, so the body's destructors never run. That is
// what a 16 KiB stack does on macOS in debug builds.
const PANIC_TEST_STACK_SIZE: usize = DEFAULT_STACK_SIZE * 2;

#[test]
fn test_return() {
    let mut g = Gn::new_scoped(|_s| 42u32);
    assert_eq!(g.next(), Some(42));
    assert!(g.is_done());
}

#[test]
fn generator_is_done() {
    let mut g = Gn::<()>::new(|| {
        yield_with(());
    });

    g.next();
    assert!(!g.is_done());
    g.next();
    assert!(g.is_done());
}

#[test]
fn generator_is_done1() {
    let mut g = Gn::new_scoped(|mut s| {
        s.yield_(2);
        done!();
    });

    assert_eq!(g.next(), Some(2));
    assert!(!g.is_done());
    assert_eq!(g.next(), None);
    assert!(g.is_done());
}

#[test]
fn generator_is_done_with_drop() {
    let mut g = Gn::new_scoped(|mut s| {
        s.yield_(String::from("string"));
        done!();
    });

    assert_eq!(g.next(), Some(String::from("string")));
    assert!(!g.is_done());
    assert_eq!(g.next(), None);
    assert!(g.is_done());
}

#[test]
fn test_yield_a() {
    let mut g = Gn::<i32>::new(|| {
        let r: i32 = yield_(10).unwrap();
        r * 2
    });

    // first start the generator
    let i = g.raw_send(None).unwrap();
    assert_eq!(i, 10);
    let i = g.send(3);
    assert_eq!(i, 6);
    assert!(g.is_done());
}

#[test]
#[should_panic(expected = "generator finished instead of yielding")]
fn test_send_after_finish() {
    let mut g = Gn::<()>::new_opt(0x800, || 1u32);
    assert_eq!(g.next(), Some(1));
    assert!(g.is_done());
    g.send(());
}

#[test]
fn test_yield_with() {
    let mut g = Gn::new(|| {
        yield_with(10);
        20
    });

    // the para type could be deduced here
    let i = g.send(());
    assert!(i == 10);

    let j = g.next();
    assert!(j.unwrap() == 20);
}

#[test]
#[should_panic]
fn test_yield_with_type_error() {
    let mut g = Gn::<()>::new(|| {
        // yield_with::<i32>(10);
        yield_with(10u32);
        20i32
    });

    g.next();
}

#[test]
#[should_panic]
fn test_get_yield_type_error() {
    let mut g = Gn::<u32>::new(|| {
        get_yield::<i32>();
    });

    g.send(10);
}

#[test]
#[should_panic]
fn test_deep_yield_with_type_error() {
    let mut g = Gn::<()>::new(|| {
        let mut g = Gn::<()>::new(|| {
            yield_with(0);
        });
        g.next();
    });

    g.next();
}

#[test]
fn test_scoped() {
    use std::cell::RefCell;
    use std::rc::Rc;

    let x = Rc::new(RefCell::new(10));

    let x1 = x.clone();
    let mut g = Gn::<()>::new_scoped_local(move |mut s| {
        *x1.borrow_mut() = 20;
        s.yield_with(());
        *x1.borrow_mut() = 5;
    });

    g.next();
    assert!(*x.borrow() == 20);

    g.next();
    assert!(*x.borrow() == 5);

    assert!(g.is_done());
}

#[test]
fn test_scoped_1() {
    let mut x = 10;
    {
        let mut g = Gn::<()>::new(|| {
            x = 5;
        });
        g.next();
    }

    assert!(x == 5);
}

#[test]
fn test_scoped_yield() {
    let mut g = Gn::new_scoped(|mut s| {
        let mut i = 0;
        loop {
            let v = s.yield_(i);
            i += 1;
            match v {
                Some(x) => {
                    // dbg!(x, i);
                    assert_eq!(x, i);
                }
                None => {
                    // for elegant exit
                    break;
                }
            }
        }
        20usize
    });

    // start g
    g.raw_send(None);

    for i in 1..100 {
        let data: usize = g.send(i);
        assert_eq!(data, i);
    }

    // quit g
    g.raw_send(None);
}

#[test]
fn test_inner_ref() {
    let mut g = Gn::<()>::new_scoped(|mut s| {
        // setup something
        let mut x: u32 = 10;

        // return internal ref not compiled because the
        // lifetime of internal ref is smaller than the generator
        // but the generator interface require the return type's
        // lifetime bigger than the generator

        // the x memory remains on heap even returned!
        // the life time of x is associated with the generator
        // however modify this internal value is really unsafe
        // but this is useful pattern for setup and teardown
        // which can be put in the same place
        unsafe {
            let mut_ref: &mut u32 = std::mem::transmute(&mut x);
            s.yield_unsafe(mut_ref);
        };

        // this was modified by the invoker
        assert!(x == 5);
        // teardown happened when the generator get dropped
        done!()
    });

    // use the resource setup from generator
    let a = g.next().unwrap();
    assert!(*a == 10);
    *a = 5;
    // a keeps valid until the generator dropped
}

#[test]
fn test_drop() {
    let mut x = 10;
    {
        let mut g = Gn::<()>::new(|| {
            x = 1;
            yield_with(());
            x = 5;
        });
        g.send(());
    }

    assert!(x == 1);
}

#[test]
fn test_ill_drop() {
    let mut x = 10u32;
    {
        Gn::<u32>::new(|| {
            x = 5;
            // here we got None from drop
            x = get_yield().unwrap_or(0);
        });
        // not started the gen, change nothing
    }

    assert!(x == 10);
}

#[test]
fn test_loop_drop() {
    let mut x = 10u32;
    {
        let mut g = Gn::<()>::new(|| -> () {
            x = 5;
            loop {
                yield_with(());
            }
        });
        g.send(());
        // here the generator drop will cancel the loop
    }

    assert!(x == 5);
}

#[test]
fn test_panic_inside() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let mut x = 10;
    {
        let mut wrapper = AssertUnwindSafe(&mut x);
        if let Err(panic) = catch_unwind(move || {
            let mut g = Gn::<()>::new(|| {
                **wrapper = 5;
                panic!("panic inside!");
            });
            g.resume();
        }) {
            match panic.downcast_ref::<&str>() {
                // why can't get the message here?? is it lost?
                Some(msg) => println!("get panic: {msg:?}"),
                None => println!("can't get panic message"),
            }
        }
        // wrapper dropped here
    }

    assert!(x == 5);
}

#[test]
#[allow(unreachable_code)]
fn test_cancel() {
    let mut g = Gn::<()>::new(|| {
        let mut i = 0;
        loop {
            yield_with(i);
            i += 1;
        }
        i
    });

    loop {
        let i = g.next().unwrap();
        if i > 10 {
            g.cancel();
            break;
        }
    }

    assert!(g.is_done());

    // a cancelled generator can be re-initialized
    g.init_code(|| 1i32);
    assert_eq!(g.next(), Some(1));
    assert!(g.is_done());
}

#[test]
fn test_cancel_not_started() {
    let mut g = Gn::<()>::new_opt(0x800, || 1u32);
    assert!(!g.is_done());

    // cancelling a generator that never ran consumes its functor
    g.cancel();
    assert!(g.is_done());
    assert_eq!(g.next(), None);

    // and the generator is still usable afterwards
    g.init_code(|| 2u32);
    assert_eq!(g.next(), Some(2));
    assert!(g.is_done());
}

#[test]
fn test_cancel_coroutine_prompts() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct Guard(Arc<AtomicBool>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    let ran = Arc::new(AtomicBool::new(false));
    let ran_in_body = ran.clone();
    let unwound = Arc::new(AtomicBool::new(false));
    let guard_unwound = unwound.clone();
    // cancelling resumes the body with a panic to unwind it, which needs the
    // same stack headroom as any other panic inside a generator
    let mut g = Gn::<()>::new_opt(PANIC_TEST_STACK_SIZE, move || {
        let _guard = Guard(guard_unwound);
        co_yield_with(0u32);
        // must not run: the coroutine is canceled while suspended here
        ran_in_body.store(true, Ordering::SeqCst);
        co_yield_with(1u32);
        2u32
    });
    // mark the generator as a coroutine
    g.set_local_data(1 as *mut u8);

    assert_eq!(g.next(), Some(0));
    g.cancel();
    assert!(g.is_done());
    assert!(!ran.load(Ordering::SeqCst));
    assert!(
        unwound.load(Ordering::SeqCst),
        "cancel did not unwind the suspended body"
    );
}

#[test]
fn test_cancel_coroutine_cannot_be_swallowed() {
    use std::panic::catch_unwind;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct Guard(Arc<AtomicBool>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    let unwound = Arc::new(AtomicBool::new(false));
    let guard_unwound = unwound.clone();
    let mut g = Gn::<()>::new_opt(PANIC_TEST_STACK_SIZE, move || {
        let _guard = Guard(guard_unwound);
        // The canceled coroutine resumes inside this call with a `Cancel`
        // panic. Swallow it and try to suspend again: the second yield must
        // re-raise `Cancel` instead of parking a body whose generator already
        // reports done.
        let _ = catch_unwind(|| co_yield_with(0u32));
        co_yield_with(1u32);
        2u32
    });
    g.set_local_data(1 as *mut u8);

    assert_eq!(g.next(), Some(0));
    g.cancel();
    assert!(g.is_done());
    assert!(
        unwound.load(Ordering::SeqCst),
        "swallowed Cancel left the body suspended"
    );
    assert_eq!(g.next(), None);
}

#[test]
fn test_drop_coroutine_does_not_run_user_code() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct Guard(Arc<AtomicBool>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    let ran = Arc::new(AtomicBool::new(false));
    let ran_in_body = ran.clone();
    let unwound = Arc::new(AtomicBool::new(false));
    let guard_unwound = unwound.clone();
    {
        // dropping resumes the body with a panic to unwind it, which needs
        // the same stack headroom as any other panic inside a generator
        let mut g = Gn::<()>::new_opt(PANIC_TEST_STACK_SIZE, move || {
            let _guard = Guard(guard_unwound);
            co_yield_with(0u32);
            ran_in_body.store(true, Ordering::SeqCst);
            1u32
        });
        g.set_local_data(1 as *mut u8);
        assert_eq!(g.next(), Some(0));
        // dropped while suspended: the body must unwind (so the guard's
        // destructor runs), not resume past the yield
    }
    assert!(!ran.load(Ordering::SeqCst));
    assert!(
        unwound.load(Ordering::SeqCst),
        "drop did not unwind the suspended body"
    );
}

#[test]
fn test_local_data() {
    static LOCAL: u8 = 0;
    let local = &LOCAL as *const u8 as *mut u8;

    let mut g = Gn::<()>::new_opt(0x800, || {
        let expected = &LOCAL as *const u8 as *mut u8;
        // the free function finds the running generator's local data
        assert_eq!(get_local_data(), expected);
        1u32
    });
    g.set_local_data(local);
    assert_eq!(g.get_local_data(), local);
    assert_eq!(g.next(), Some(1));
}

#[test]
fn test_coroutine_panic_data() {
    let mut g: Generator<'_, (), u32> = Gn::<()>::new_opt(PANIC_TEST_STACK_SIZE, || panic!("boom"));
    g.set_local_data(1 as *mut u8);
    assert_eq!(g.next(), None);
    assert!(g.is_done());

    let panic = g.get_panic_data().expect("coroutine panic data");
    // the payload of `panic!` is `&'static str` for a bare literal and a
    // `String` for a formatted message, so accept either representation
    let msg = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str));
    assert_eq!(msg, Some("boom"));

    // the error is consumed
    assert!(g.get_panic_data().is_none());
    assert_eq!(g.next(), None);
}

#[test]
#[should_panic]
fn test_yield_from_functor_context() {
    // this is not run from generator
    yield_::<(), _>(0);
}

#[test]
#[should_panic]
fn test_yield_with_from_functor_context() {
    // this is not run from generator
    yield_with(0);
}

#[test]
fn test_yield_from_generator_context() {
    let mut g = Gn::<()>::new(|| {
        let mut g1 = Gn::<()>::new(|| {
            yield_with(5);
            10
        });

        let i = g1.send(());
        yield_with(i);
        0
    });

    let n = g.send(());
    assert!(n == 5);

    let n = g.send(());
    assert!(n == 0);
}

#[test]
fn test_yield_from() {
    let mut g = Gn::<()>::new(|| {
        let g1 = Gn::<()>::new(|| {
            yield_with(5);
            10
        });

        yield_from(g1);
        0
    });

    let n = g.send(());
    assert!(n == 5);
    let n = g.send(());
    assert!(n == 10);
    let n = g.send(());
    assert!(n == 0);
    assert!(g.is_done());
}

#[test]
fn test_yield_from_send() {
    let mut g = Gn::<u32>::new(|| {
        let g1 = Gn::<u32>::new(|| {
            let mut i: u32 = yield_(1u32).unwrap();
            i = yield_(i * 2).unwrap();
            i * 2
        });

        let i = yield_from(g1).unwrap();
        assert_eq!(i, 10);

        // here we need a unused return to indicate this function's return type
        0u32
    });

    // first start the generator
    let n = g.raw_send(None).unwrap();
    assert!(n == 1);

    let n = g.send(3);
    assert!(n == 6);
    let n = g.send(4);
    assert!(n == 8);
    let n = g.send(10);
    assert!(n == 0);
    assert!(g.is_done());
}

#[test]
#[should_panic]
fn test_yield_from_send_type_miss_match() {
    let mut g = Gn::<u32>::new(|| {
        let g1 = Gn::<u32>::new(|| {
            let mut i: u32 = yield_(1u32).unwrap();
            i = yield_(i * 2).unwrap();
            i * 2
        });

        yield_from(g1);
        // here the return type should be 0u32
        0
    });

    let n = g.send(3);
    assert!(n == 1);
    let n = g.send(4);
    assert!(n == 6);
    let n = g.send(10);
    assert!(n == 8);
    // the last send has no meaning for the return
    let n = g.send(0);
    assert!(n == 0);
    assert!(g.is_done());
}

// windows has it's own check, this test would make the app abort
// #[test]
// #[should_panic]
// fn test_stack_overflow() {
//     // here the stack size is not big enough
//     // and will panic when get detected in drop
//     let clo = || {
//         let big_data = [0usize; 0x400];
//         println!("this would overflow the stack, {}", big_data[100]);
//     };
//     Gn::<()>::new_opt(clo, 10);
// }

#[test]
#[should_panic(expected = "allocation exceeds stack")]
fn test_stack_box_oversized_functor_panics() {
    // The closure captures 16 KiB while the stack is only one page. This used
    // to compute a pointer below the stack mapping (and crash with SIGSEGV,
    // or corrupt unrelated memory); it must fail cleanly instead.
    let big = [0u8; 1 << 14];
    let _g = Gn::<()>::new_opt(16, move || {
        std::hint::black_box(big[0]);
        0u32
    });
}

#[test]
fn test_stack_box_large_functor_fits() {
    // The same 16 KiB capture must still be accepted when the stack is big
    // enough, so the new bounds check does not reject valid generators.
    let big = [0u8; 1 << 14];
    let mut g = Gn::<()>::new_opt(0x20000, move || {
        std::hint::black_box(big[0]);
        42u32
    });
    assert_eq!(g.next(), Some(42));
    assert!(g.is_done());
}

#[cfg(all(not(target_os = "fuchsia"), not(windows)))]
#[test]
fn test_nested_stack_overflow() {
    use std::panic::catch_unwind;

    // The inner generator overflows its own stack while the outer generator is
    // running. The overflow handler must validate the fault against the inner
    // (currently executing) stack guard, not the outermost generator's.
    let result = catch_unwind(|| {
        let mut outer = Gn::new_scoped(|_s: Scope<(), ()>| {
            let mut inner = Gn::<()>::new_opt(16, || overflow_stack(0));
            inner.next();
        });
        outer.next();
    });

    assert!(matches!(
        result.map_err(|err| *err.downcast::<Error>().unwrap()),
        Err(Error::StackErr)
    ));
}

#[cfg(all(not(target_os = "fuchsia"), not(windows)))]
#[test]
fn test_caught_stack_overflow_drops_cleanly() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let mut g = Gn::new_scoped(|_s: Scope<(), usize>| overflow_stack(0));

    let result = catch_unwind(AssertUnwindSafe(|| g.next()));
    assert!(matches!(
        result.map_err(|err| *err.downcast::<Error>().unwrap()),
        Err(Error::StackErr)
    ));

    // The overflow was already reported by `next`; dropping the generator
    // must not panic a second time, even though the abandoned frames leave
    // the stack bookkeeping inconsistent.
    drop(g);
}

#[test]
fn test_scope_gen() {
    // now we can even deduce the input para type
    let mut g = Gn::new_scoped(|mut s| {
        let i = s.yield_(0).unwrap();
        // below would have a compile error, nice!
        // s.yield_(Box::new(0));
        i * 2
    });

    assert_eq!(g.raw_send(None), Some(0));
    assert_eq!(g.raw_send(Some(3)), Some(6));
    assert_eq!(g.raw_send(None), None);
}

#[test]
fn test_scoped_opt_constructors() {
    // the size is in `usize` words, matching DEFAULT_STACK_SIZE
    let mut g = Gn::<()>::new_scoped_opt(0x800, |mut s: Scope<(), u32>| {
        s.yield_with(1);
        2
    });
    assert_eq!(g.next(), Some(1));
    assert_eq!(g.next(), Some(2));
    assert!(g.is_done());

    let mut l = Gn::<()>::new_scoped_opt_local(0x800, |mut s: Scope<(), u32>| {
        s.yield_with(3);
        4
    });
    assert_eq!(l.next(), Some(3));
    assert_eq!(l.next(), Some(4));
    assert!(l.is_done());
}

#[test]
fn test_scope_yield_from_send() {
    let mut g = Gn::new_scoped(|mut s| {
        let g1 = Gn::new_scoped(|mut s| {
            let mut i: u32 = s.yield_(1u32).unwrap();
            i = s.yield_(i * 2).unwrap();
            i * 2
        });

        let i = s.yield_from(g1).unwrap();
        // here the return type should be 0u32
        i * 2
    });

    let n = g.send(3);
    assert_eq!(n, 1);
    let n = g.send(4);
    assert_eq!(n, 8);
    let n = g.send(10);
    assert_eq!(n, 20);
    // the last send has no meaning for the return
    let n = g.send(7);
    assert!(n == 14);
    assert!(g.is_done());
}

#[test]
fn test_re_init() {
    let clo = || {
        |mut s: Scope<'_, 'static, (), _>| {
            s.yield_(0);
            s.yield_(3);
            5
        }
    };

    let mut g = Gn::new_opt(0x800, || 0);
    g.scoped_init(clo());

    assert_eq!(g.next(), Some(0));
    assert_eq!(g.next(), Some(3));
    assert_eq!(g.next(), Some(5));
    assert!(g.is_done());

    // re-init generator
    g.scoped_init(clo());

    assert_eq!(g.next(), Some(0));
    assert_eq!(g.next(), Some(3));
    assert_eq!(g.next(), Some(5));
    assert!(g.is_done());
}

#[test]
fn test_re_init_after_failed_allocation() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    // Re-initializing with a closure too large for the stack panics inside
    // `init_code`. If that panic is caught, the generator must be left done
    // (it has no function installed) instead of resumable with stale
    // registers.
    let mut g = Gn::<()>::new_opt(0x800, || 0u32);
    assert_eq!(g.next(), Some(0));
    assert!(g.is_done());

    // 32 KiB capture vs a 16 KiB stack: the allocation must fail. Keep the
    // array small enough not to overflow the test thread's own stack.
    let big = [0u8; 1 << 15];
    let result = catch_unwind(AssertUnwindSafe(|| {
        g.init_code(move || {
            std::hint::black_box(big[0]);
            1u32
        });
    }));
    assert!(result.is_err(), "the oversized functor must panic");

    // the generator has no function installed, so it must not be resumable
    assert!(g.is_done());
    assert_eq!(g.next(), None);

    // and it must still be re-initializable afterwards
    g.init_code(|| 2u32);
    assert_eq!(g.next(), Some(2));
    assert!(g.is_done());
}

#[test]
fn test_re_init_clears_stale_para() {
    // The first run never consumes the para that was set before it started.
    let mut g = Gn::new_scoped(|mut s: Scope<u32, u32>| {
        s.yield_with(0);
        1
    });
    g.set_para(7);
    assert_eq!(g.resume(), Some(0));
    assert_eq!(g.resume(), Some(1));
    assert!(g.is_done());

    // the new run must not receive the stale para
    g.scoped_init(|mut s: Scope<u32, u32>| {
        s.yield_with(0);
        assert_eq!(s.get_yield(), None, "stale para leaked into the new run");
        2
    });
    assert_eq!(g.resume(), Some(0));
    assert_eq!(g.resume(), Some(2));
    assert!(g.is_done());
}

#[test]
fn test_re_init_after_cancel_clears_stale_para() {
    // The first run yields without consuming the para queued before the
    // resume, and is then canceled while suspended.
    let mut g = Gn::new_scoped(|mut s: Scope<u32, u32>| {
        s.yield_with(0);
        1
    });
    g.set_para(7);
    assert_eq!(g.resume(), Some(0));
    assert!(!g.is_done());

    g.cancel();
    assert!(g.is_done());

    // the new run must not receive the para the canceled run never consumed
    g.scoped_init(|mut s: Scope<u32, u32>| {
        s.yield_with(0);
        assert_eq!(s.get_yield(), None, "stale para leaked into the new run");
        2
    });
    assert_eq!(g.resume(), Some(0));
    assert_eq!(g.resume(), Some(2));
    assert!(g.is_done());
}

#[test]
fn test_re_init_clears_panic_data() {
    let mut g: Generator<'_, (), u32> = Gn::<()>::new_opt(PANIC_TEST_STACK_SIZE, || panic!("boom"));
    // mark it as a coroutine, so the panic is stored instead of being
    // re-raised by `resume`
    g.set_local_data(1 as *mut u8);
    assert_eq!(g.next(), None);
    assert!(g.is_done());

    // Re-init without taking the panic data: the stale error must be cleared
    // while the generator is still a coroutine, rather than resurfacing via
    // `get_panic_data` or being re-raised by the next resume.
    g.init_code(|| 1u32);
    assert!(
        g.get_panic_data().is_none(),
        "re-init kept the old panic data for a coroutine"
    );
    g.set_local_data(std::ptr::null_mut());
    assert_eq!(g.next(), Some(1));
    assert!(g.is_done());
}

#[test]
fn test_re_init_after_panic() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let mut g: Generator<'_, (), u32> = Gn::<()>::new_opt(PANIC_TEST_STACK_SIZE, || panic!("boom"));
    let result = catch_unwind(AssertUnwindSafe(|| g.next()));
    assert!(result.is_err());
    assert!(g.is_done());
    // `resume` re-raised and consumed the panic, so nothing is left here
    assert!(g.get_panic_data().is_none());

    // a generator that panicked can be re-initialized like a finished one
    g.init_code(|| 1u32);
    assert_eq!(g.next(), Some(1));
    assert!(g.is_done());
}

#[test]
fn test_re_init_while_suspended() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct Guard(Arc<AtomicBool>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    let unwound = Arc::new(AtomicBool::new(false));
    let guard_unwound = unwound.clone();
    let mut g = Gn::new_scoped(move |mut s: Scope<u32, u32>| {
        let _guard = Guard(guard_unwound);
        s.yield_with(1);
        s.yield_with(2);
        3
    });
    assert_eq!(g.resume(), Some(1));
    assert!(!g.is_done());
    assert!(!unwound.load(Ordering::SeqCst));

    // the old run is still suspended: re-initializing must cancel it first,
    // which unwinds the suspended body and runs its destructors
    g.scoped_init(|mut s: Scope<u32, u32>| {
        s.yield_with(10);
        11
    });
    assert!(
        unwound.load(Ordering::SeqCst),
        "suspended run was not cancelled"
    );
    assert_eq!(g.resume(), Some(10));
    assert_eq!(g.resume(), Some(11));
    assert!(g.is_done());
}

#[test]
#[should_panic]
fn done_in_normal() {
    done!();
}

#[test]
#[should_panic]
fn invalid_yield_in_scope() {
    let g = Gn::new_scoped(|_| {
        // invalid use raw yield API with scope
        yield_::<String, _>(());
    });

    for () in g {}
}

#[test]
fn test_yield_float() {
    let mut g = Gn::<f64>::new(|| {
        let r: f64 = yield_(10.0).unwrap();
        let x = r * 2.0; // 6
        let y = x * 9.0; // 54
        let z = y / 3.0; // 18
        let r: f64 = yield_(6.0).unwrap();
        x * r * y * z
    });

    // first start the generator
    let i = g.raw_send(None).unwrap();
    let x = i * 10.0;
    assert_eq!(i, 10.0);
    let i = g.send(3.0);
    assert_eq!(i, 6.0);
    let i = g.send(x / 25.0);
    assert_eq!(i, 23328.0);
    assert!(g.is_done());
}

#[test]
fn test_stack_usage_tracking() {
    // even size: only the bottom words are sampled, so the estimate is an
    // upper bound close to the total
    let g = Gn::<()>::new_opt(0x800, || 0u32);
    let (total, used) = g.stack_usage();
    assert!(used + 8 >= total, "used={used}, total={total}");

    // odd size: full tracking, so an almost unused stack reports a small usage
    let g = Gn::<()>::new_opt(0x801, || 0u32);
    let (total, used) = g.stack_usage();
    assert!(used < total / 2, "used={used}, total={total}");
}

#[test]
fn test_generator_moves_across_threads() {
    let mut g = Gn::<()>::new_scoped(|mut s| {
        s.yield_(1u32);
        s.yield_(2u32);
        3u32
    });
    assert_eq!(g.next(), Some(1));

    // Moving the suspended generator also moves access to every stack handle;
    // the owning thread no longer touches it.
    let handle = std::thread::spawn(move || {
        assert_eq!(g.next(), Some(2));
        assert_eq!(g.next(), Some(3));
        assert!(g.is_done());
    });
    handle.join().unwrap();
}

#[test]
fn test_into_raw_from_raw() {
    let g = Gn::<()>::new_scoped(|mut s| {
        s.yield_(1u32);
        s.yield_(2u32);
        3u32
    });

    // the raw pointer owns the generator (and its stack) until it is
    // reconstructed
    let raw = g.into_raw();
    let mut g = unsafe { Generator::<(), u32>::from_raw(raw) };

    assert_eq!(g.next(), Some(1));
    assert_eq!(g.next(), Some(2));
    assert_eq!(g.next(), Some(3));
    assert!(g.is_done());

    // the local generator round-trips through the same API
    let g: LocalGenerator<'_, (), u32> = Gn::<()>::new_scoped_local(|mut s: Scope<(), u32>| {
        s.yield_with(4);
        5
    });
    let raw = g.into_raw();
    let mut g = unsafe { LocalGenerator::<(), u32>::from_raw(raw) };
    assert_eq!(g.next(), Some(4));
    assert_eq!(g.next(), Some(5));
    assert!(g.is_done());
}
