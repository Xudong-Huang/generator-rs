#[macro_use]
extern crate generator;
use generator::{Generator, get_yield, yield_with};

fn sum(a: u32) -> u32 {
    let mut sum = a;
    let mut recv: u32;
    while sum < 200 {
        recv = get_yield().unwrap();
        yield_with(sum);
        sum += recv;
    }

    sum
}

fn main() {
    // we specify the send type is u32
    let mut s = generator!(sum(1), <u32>);
    let mut i = 1u32;
    while !s.is_done() {
        i = s.send(i);
        println!("{}", i);
    }
}
