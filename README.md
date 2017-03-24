[![Build Status](https://travis-ci.org/Xudong-Huang/generator-rs.svg?branch=master)](https://travis-ci.org/Xudong-Huang/generator-rs)

# Generator-rs

rust generator library

you need the nightly rust compiler to compile it

use the dev version on master

```toml
[dependencies.generator]
git = "https://github.com/Xudong-Huang/generator-rs.git"
```


## Usage
```rust
#[macro_use]
extern crate generator;
use generator::Gn;

fn main() {
    let g = Gn::new_scoped(|mut s| {
        let (mut a, mut b) = (0, 1);
        while b < 200 {
            std::mem::swap(&mut a, &mut b);
            b = a + b;
            s.yield_(b);
        }
        done!();
    });

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
```

## Goals

- [x] basic send/yield with message support
- [x] generator cancel support
- [x] yield_from support
- [x] panic inside genertor support
- [x] stack size tune support
- [x] scoped static type support
- [x] basic coroutine interface support
- [ ] stable rust support


##  based on this basic library
- we can easily port python libary based on generator into rust. the libaray has more powerfull tools than python that can make the generator more fast, safe and multi thread support.
- schedule framework running with multi-thread.
- basic library for Asynchronous I/O


## Notices

* This crate supports platforms in

    - x86_64 linux
    - x86_64 windows
