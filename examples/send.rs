extern crate generator;
use generator::{Gn, yield_};

fn sum(a: u32) -> u32 {
    let mut sum = a;
    let mut recv = 1u32;
    while sum < 200 {
        sum += recv;
        recv = yield_(sum).unwrap();
    }

    sum + recv
}

fn main() {
    // we specify the send type is u32
    let mut s = Gn::<u32>::new(|| sum(0));
    let mut i = 1u32;
    while !s.is_done() {
        i = s.send(i);
        println!("{}", i);
    }
}
