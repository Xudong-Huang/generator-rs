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
#![feature(repr_simd)]

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
mod reg_context;

pub use rt::Error;
pub use scope::Scope;
pub use rt::{get_local_data, is_generator};
pub use gen_impl::{Gn, Generator, GeneratorImpl};
pub use yield_::{yield_, yield_with, yield_from, get_yield, co_yield_with, co_get_yield, done};
