//! # generator
//!
//! Rust generator library
//!

#![feature(asm)]
#![feature(alloc)]
#![feature(fnbox)]
#![feature(rustc_private)]
#![feature(core_intrinsics)]

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
pub use yield_::{yield_, yield_with, yield_from, get_yield};
