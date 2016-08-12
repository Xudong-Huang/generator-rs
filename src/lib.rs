//! # generator
//!
//! Rust generator library
//!

#![feature(asm)]
#![feature(alloc)]
#![feature(fnbox)]
#![feature(rustc_private)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]

#![cfg_attr(test, deny(warnings))]
#![deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate alloc;

mod rt;
mod scope;
mod stack;
mod detail;
mod yield_;
mod gen_impl;
// mod generator;
mod reg_context;

pub use scope::Scope;
pub use gen_impl::{Gn, Generator, GeneratorImpl};
pub use yield_::{yield_, yield_with, yield_from, get_yield};
