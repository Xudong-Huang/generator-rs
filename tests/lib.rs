extern crate generator;

use generator::*;

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
    use std::rc::Rc;
    use std::cell::RefCell;
    let x = Rc::new(RefCell::new(10));

    let x1 = x.clone();
    let mut g = Gn::<()>::new(move || {
        *x1.borrow_mut() = 20;
        yield_with(());
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
fn test_inner_ref() {
    use std::mem;
    let mut g = Gn::<()>::new(|| -> &mut u32 {
        // setup something
        let mut x: u32 = 10;

        // the x memory remains on heap even returned!
        // the life time of x is assosiated with the generator
        // however modify this interal value is really unsafe
        // but this is useful pattern for setup and teardown
        // which can be put in the same place
        {
            // mut borrow block
            let y: &mut u32 = unsafe { mem::transmute(&mut x) };
            yield_with(y);
        }
        // this was modified by the invoker
        assert!(x == 5);
        // teardown happened when the generator get dropped
        unsafe { mem::transmute(&mut x) }
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
        Gn::<()>::new(|| {
            x = 1;
            yield_with(());
            x = 5;
        });
    }

    assert!(x == 5);
}

#[test]
fn test_ill_drop() {
    let mut x = 10u32;
    {
        Gn::<u32>::new(|| {
            x = 5;
            // here we got None from drop
            // but should no panic
            x = get_yield().unwrap();
        });
    }

    assert!(x == 5);
}

#[test]
fn test_loop_drop() {
    let mut x = 10u32;
    {
        Gn::<()>::new(|| {
            x = 5;
            loop {
                yield_with(());
            }
        });
        // here the generator drop will cancel the loop
    }

    assert!(x == 5);
}

#[test]
fn test_panic_inside() {
    let mut x = 10;
    {
        Gn::<()>::new(|| {
            x = 5;
            panic!("panic inside!");
        });
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
}

#[test]
#[should_panic]
fn test_yield_from_functor_context() {
    // this is not run from generator
    yield_::<(), _>(0);
}

#[test]
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

        yield_from(g1);

        // here we need a unused return to indicate this function's return type
        0u32
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
}/*

// windows has it's own check, this test would make the app abort
#[test]
#[should_panic]
fn test_stack_overflow() {
    // here the stack size is not big enough
    // and will panic when get detected in drop
    let clo = || {
        let big_data = [0usize; 0x400];
        println!("this would overflow the stack, {}", big_data[100]);
    };
    Gn::<()>::new_opt(clo, 10);
} */
