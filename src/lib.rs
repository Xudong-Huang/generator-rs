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
mod fn_gen;
mod yield_;
mod generator;
mod reg_context;

pub use generator::Generator;
pub use fn_gen::FnGenerator;
pub use yield_::{yield_with, get_yield, yield_from};
