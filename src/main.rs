#[macro_use]
extern crate generator;
use generator::Generator;

unsafe fn f0() {
    let mut i = 0;
    println!("{}", i);

    _yield_!();

    i = 100;
    println!("{}", i);

    _yield_!();

    i = 1000;
    println!("{}", i);
}


unsafe fn f1() -> u32 {
    let mut j = 0;
    let mut i:Option<i32>;
    while j < 10 {
        i = _yield!(j);
        println!("get send: {:?}", i);
        j+=1;
    }

    return 10000;
}

unsafe fn f2() -> (u64, u64, u64)
{
    let mut i = 0u64;
    while i < 10 {
        _yield_!((i, i+1, i+2));
        i+=1;
    }

    // the last return is not deal with carefully
    (0, 0, 0)
}


fn main() {
    let mut g = generator!(f0());

    g.next();
    g.next();
    g.next();

    let mut g = generator!(f1(), <i32>);
    let mut i = 0;
    while !g.is_done() {
        println!("get yield: {:?}", g.send(Some(i)));
        i += 1;
    }

    let g = generator!(f2());
    for x in g {
        println!("get{:?}", x);
    }
}
