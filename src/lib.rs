//! # generator
//!
//! Rust generator library
//!

#![feature(asm)]
#![feature(alloc)]
#![feature(fnbox)]
#![feature(repr_simd)]
#![feature(catch_panic)]
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
// mod stack_size;
mod reg_context;

pub use generator::{Generator, Gn};
pub use yield_::{yield_with, get_yield, yield_from};
