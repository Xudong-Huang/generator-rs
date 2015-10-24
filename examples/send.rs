#[macro_use]
extern crate generator;
use generator::Generator;

fn sum(a: u32) -> u32 {
    let mut sum = a;
    let mut recv = 1u32;
    while sum < 200 {
        sum += recv;
        recv = _yield!(sum);
    }

    10000
}

fn main() {
    // we specify the send type is u32
    let mut s = generator!(sum(0), <u32>);
    let mut i = 0u32;
    while !s.is_done() {
        i = s.send(i);
        println!("{}", i);
    }
}
