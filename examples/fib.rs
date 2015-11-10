extern crate generator;
use generator::{Gn, yield_with};

fn main() {

    let g = Gn::<()>::new(|| {
        let (mut a, mut b) = (0, 1);
        while b < 200 {
            std::mem::swap(&mut a, &mut b);
            b = a + b;
            yield_with(b);
        }
        a + b
    });

    for i in g {
        println!("{}", i);
    }
}
