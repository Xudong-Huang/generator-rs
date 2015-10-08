#[macro_use]
extern crate generator;

use generator::{Generator, make_gen};

fn f1() -> u32 {
    let mut j = 0;
    let mut i:i32;
    while j < 10 {
        i = _yield!(j);
        println!("get send: {:?}", i);
        j+=1;
    }

    return 10000;
}

fn f2() -> (u64, u64, u64)
{
    let mut i = 0u64;
    while i < 10 {
        _yield_!((i, i+1, i+2));
        i+=1;
    }

    // the last return is not deal with carefully
    (0, 0, 0)
}

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
    let mut g = make_gen::<(), _>(None, Box::new(||{
        _yield_!(10);
        20
    }));

    let i = g.send(());
    assert!(i == 10);

    let j = g.next();
    assert!(j.unwrap() == 20);
}



#[test]
fn test_main() {
    let mut g = generator!(f1(), <i32>);
    let mut i = 0;
    while !g.is_done() {
        println!("get yield: {:?}", g.send(i));
        i += 1;
    }

    let g = generator!(f2());
    for x in g {
        println!("get{:?}", x);
    }
}
