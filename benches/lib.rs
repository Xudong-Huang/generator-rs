#![feature(test)]
extern crate generator;
extern crate test;

use std::panic;
use generator::*;
use test::Bencher;

#[bench]
fn yield_bench(b: &mut Bencher) {
    // don't print any panic info
    panic::set_hook(Box::new(|_| {}));

    b.iter(|| {
        let mut g = Gn::new(|| {
            for i in 0.. {
                yield_with(i);
            }
            20
        });

        for i in 0..1000_000 {
            let data = g.send(());
            assert_eq!(data, i);
        }
    });
}

#[bench]
fn single_yield_bench(b: &mut Bencher) {
    // don't print any panic info
    panic::set_hook(Box::new(|_| {}));

    let mut g = Gn::new(|| {
        for i in 0.. {
            yield_with(i);
        }
        20
    });

    let mut i = 0;
    b.iter(|| {
        let data = g.send(());
        assert_eq!(data, i);
        i += 1;
    });
}
