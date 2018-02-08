// #![feature(conservative_impl_trait)]
#[macro_use]
extern crate generator;
use generator::*;

fn main() {
    // fn square<'a, T: Iterator<Item = u32> + 'a>(input: T) -> impl Iterator<Item = u32> + 'a {
    fn square<'a, T: Iterator<Item = u32> + 'a>(input: T) -> Generator<'a, (), u32> {
        Gn::new_scoped(|mut s| {
            for i in input {
                println!("square input = {}", i);
                s.yield_with(i * i);
            }
            done!();
        })
    }

    // fn sum<'a, T: Iterator<Item = u32> + 'a>(input: T) -> impl Iterator<Item = u32> + 'a {
    fn sum<'a, T: Iterator<Item = u32> + 'a>(input: T) -> Generator<'a, (), u32> {
        Gn::new_scoped(|mut s| {
            let mut acc = 0;
            for i in input {
                println!("sum input = {}", i);
                acc += i;
                s.yield_with(acc);
            }
            done!();
        })
    }

    for (i, sum) in sum(square(0..20)).enumerate() {
        println!("square_sum_{:<2} = {:^4}", i, sum);
    }
}
