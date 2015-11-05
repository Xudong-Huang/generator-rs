#[macro_use]
extern crate generator;

fn fib(a: u32, b: u32) -> u32 {
    let (mut a, mut b) = (a, b);
    while b < 200 {
        std::mem::swap(&mut a, &mut b);
        b = a + b;
        _yield_!(b);
    }

    a + b
}

fn main() {

    let g = generator!(fib(0, 1));

    for i in g {
        println!("{}", i);
    }
}
