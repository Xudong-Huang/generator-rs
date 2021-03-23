[![Build Status](https://travis-ci.org/Xudong-Huang/generator-rs.svg?branch=master)](https://travis-ci.org/Xudong-Huang/generator-rs)
[![Current Crates.io Version](https://img.shields.io/crates/v/generator.svg)](https://crates.io/crates/generator)
[![Document](https://img.shields.io/badge/doc-generator-green.svg)](https://docs.rs/generator)


# Generator-rs

rust stackfull generator library

```toml
[dependencies]
generator = "0.6"
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
- [x] panic inside generator support
- [x] stack size tune support
- [x] scoped static type support
- [x] basic coroutine interface support
- [x] stable rust support


##  based on this basic library
- we can easily port python library based on generator into rust
- coroutine framework running on multi thread


## Notices

* This crate supports below platforms, welcome to contribute with other arch and platforms

    - x86_64 Linux
    - x86_64 MacOs
    - x86_64 Windows
    - aarch64 Linux

## License

This project is licensed under either of the following, at your option:

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)