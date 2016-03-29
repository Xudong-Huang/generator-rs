//! # generator
//!
//! Rust generator library
//!

#![feature(asm)]
#![feature(alloc)]
#![feature(fnbox)]
#![feature(std_panic)]
#![feature(recover)]
#![feature(repr_simd)]
#![feature(rustc_private)]

#![cfg_attr(test, deny(warnings))]
#![deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate alloc;

mod rt;
mod stack;
mod detail;
mod yield_;
mod gen_impl;
mod generator;
mod reg_context;

pub use generator::Generator;
pub use gen_impl::{Gn, GeneratorImpl};
pub use yield_::{yield_with, get_yield, yield_from, yield_};
