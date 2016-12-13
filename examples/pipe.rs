#![feature(conservative_impl_trait)]
#[macro_use]
extern crate generator;
use generator::*;

fn main() {
    fn square<'a, T: Iterator<Item = u32> + 'a>(input: T) -> impl Iterator<Item = u32> + 'a {
        Gn::new_scoped(|mut s| {
            for i in input {
                s.yield_with(i * i);
            }
            done!();
        })
    }

    fn sum<'a, T: Iterator<Item = u32> + 'a>(input: T) -> impl Iterator<Item = u32> + 'a {
        Gn::new_scoped(|mut s| {
            let mut acc = 0;
            for i in input {
                acc += i;
                s.yield_with(acc);
            }
            done!();
        })
    }

    for i in sum(square(0..10)) {
        println!("i = {:?}", i);
    }
}
