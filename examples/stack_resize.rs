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
    let g = get_gen();
    println!("stack size is: {}", g.get_stack_size());

    for i in g {
        println!("{}", i);
    }

    let g = get_gen();
    println!("new stack size is: {}", g.get_stack_size());

    for i in g {
        println!("{}", i);
    }
}
