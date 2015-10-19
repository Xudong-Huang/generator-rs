#[macro_use]
extern crate generator;

use generator::{Generator, FnGenerator};

#[test]
fn generator_is_done() {
    let mut g = generator!({
        _yield_!();
    });

    g.next();
    assert!(!g.is_done());
    g.next();
    assert!(g.is_done());
}

#[test]
fn test_yield() {
    let mut g = FnGenerator::new(|| {
        _yield_!(10);
        20
    });

    // the para type could be deduced here
    let i = g.send(());
    assert!(i == 10);

    let j = g.next();
    assert!(j.unwrap() == 20);
}

#[test]
fn test_scoped() {
    use std::rc::Rc;
    use std::cell::RefCell;
    let x = Rc::new(RefCell::new(10));

    let x1 = x.clone();
    let mut g = FnGenerator::<(), _>::new(move|| {
         *x1.borrow_mut() = 20;
         _yield_!();
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
        let mut g = FnGenerator::<(), _>::new( || {
           x = 5;
        });
        g.next();
    }

    assert!(x == 5);
}
