# Generator-rs

rust generator library

use the dev version on master

```toml
[dependencies.generator]
git = "https://github.com/Xudong-Huang/generator-rs.git"
```


## Usage
```rust
#[macro_use]
extern crate generator;

unsafe fn fib(a: u32, b: u32) -> u32 {
    let (mut a, mut b) = (a, b);
    while b < 200 {
        std::mem::swap(&mut a, &mut b);
        b = a + b;
        _yield_!(b);
    }
    10000
}

fn main() {
    let g = generator!(fib(0, 1));

    for i in g {
        println!("{}", i);
    }
}

```

## Output
```
1
2
3
5
8
13
21
34
55
89
144
233
10000
```

## Goals

- [x] Basic single threaded support
- [ ] Stack Cache support 
- [ ] Multithreaded support
- [ ] 1M+ generators running with N:M
- [ ] basic library for Asynchronous I/O

## Notices

* This crate supports platforms in

    - x86_64

* It depends on the contex libaray, currently the context library need
  some patch to compile the generator lib

