#[macro_use]
extern crate generator;
use generator::Gn;
use generator::Generator;

fn fib(a: u32, b: u32) -> u32 {
    let (mut a, mut b) = (a, b);
    while b < 200 {
        std::mem::swap(&mut a, &mut b);
        b = a + b;
        _yield_!(b);
    }

    a + b
}

fn get_gen() -> Box<Generator<(), Output = u32>> {
    Gn::<()>::new(|| fib(0, 1))
}

fn main() {
    let mut g = get_gen();

    while !g.is_done() {
        println!("{}", g.send(()));
    }
    println!("stack size is: {:?}", g.stack_usage());


    let mut g = get_gen();
    while !g.is_done() {
        println!("{}", g.send(()));
    }
    println!("stack size is: {:?}", g.stack_usage());
}
