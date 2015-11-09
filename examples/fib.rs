#[macro_use]
extern crate generator;

fn main() {

    let g = generator::Gn::<()>::new(|| {
        let (mut a, mut b) = (0, 1);
        while b < 200 {
            std::mem::swap(&mut a, &mut b);
            b = a + b;
            _yield_!(b);
        }
        a + b
    });

    for i in g {
        println!("{}", i);
    }
}
