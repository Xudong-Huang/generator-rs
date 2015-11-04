//! # generator
//!
//! Rust generator library
//!

#![feature(fnbox)]
#![feature(asm)]
#![feature(alloc)]
#![feature(libc)]
#![feature(repr_simd)]
#![feature(rustc_private)]
#![feature(catch_panic)]
#![cfg_attr(test, deny(warnings))]
#![deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate libc;
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
pub use yield_::{yield_with, get_yield};
