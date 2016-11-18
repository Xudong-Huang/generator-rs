#![feature(test)]
#![feature(fnbox)]
extern crate generator;
extern crate test;

use std::panic;
use generator::*;
use test::Bencher;

// #[bench]
#[allow(dead_code)]
fn yield_bench(b: &mut Bencher) {
    // don't print any panic info
    // when cancel the generator
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
fn single_yield_with_bench(b: &mut Bencher) {
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

#[bench]
fn single_yield_bench(b: &mut Bencher) {
    let mut g = Gn::new(|| {
        let mut i = 0;
        loop {
            let v: Option<usize> = yield_(i);
            i += 1;
            match v {
                Some(x) => assert_eq!(x, i),
                // for elegant exit
                None => break,
            }
        }
        20usize
    });

    // start g
    g.raw_send(None);

    let mut i: usize = 1;
    b.iter(|| {
        let data: usize = g.send(i);
        assert_eq!(data, i);
        i += 1;
    });

    // quit g
    g.raw_send(None);
}

#[bench]
fn scoped_yield_bench(b: &mut Bencher) {
    let mut g = Gn::new_scoped(|mut s| {
        let mut i = 0;
        loop {
            let v = s.yield_(i);
            i += 1;
            match v {
                Some(x) => {
                    assert_eq!(x, i);
                }
                None => {
                    // for elegant exit
                    break;
                }
            }
        }
        20usize
    });

    // start g
    g.raw_send(None);

    let mut i: usize = 1;
    b.iter(|| {
        let data: usize = g.send(i);
        assert_eq!(data, i);
        i += 1;
    });

    // quit g
    g.raw_send(None);
}

#[bench]
fn create_gen(b: &mut Bencher) {
    b.iter(|| {
        let g = Gn::<()>::new_scoped(|mut s| {
            let mut i = 0;
            loop {
                match s.yield_(i) {
                    Some(..) => {
                        i += 1;
                    }
                    None => {
                        break;
                    }
                }
            }
            i
        });
        test::black_box(g)
    });
}

#[bench]
fn init_gen(b: &mut Bencher) {
    let clo_gen = || {
        |mut s: Scope<(), _>| {
            let mut i = 0;
            loop {
                match s.yield_(i) {
                    Some(..) => {
                        i += 1;
                    }
                    None => {
                        i += 1;
                        break;
                    }
                }
            }
            i
        }
    };

    let mut g = Gn::<()>::new_scoped(clo_gen());
    assert_eq!(g.raw_send(None), Some(0)); // start
    assert_eq!(g.raw_send(None), Some(1)); // cancel
    assert_eq!(g.is_done(), true);

    b.iter(|| {
        let s = g.get_scope();
        let clo = clo_gen();
        // this cost about 20ns on unix and 60ns on windows
        // because windows Box::new take more time
        g.init(move || clo(s));
        // this cost about 70ns
        // assert_eq!(g.next(), Some(0));
    });
}


#[bench]
fn fnbox_bench(b: &mut Bencher) {
    use std::boxed::FnBox;

    b.iter(|| {
        let a: [usize; 100] = [0; 100];
        let f: Box<FnBox()> = Box::new(|| {
            test::black_box(a);
        });
        test::black_box(f);
    });
}
