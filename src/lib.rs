//! # generator
//!
//! Rust generator library
//!

#![cfg_attr(nightly, feature(asm))]
#![cfg_attr(nightly, feature(repr_simd))]
#![cfg_attr(nightly, feature(core_intrinsics))]
#![cfg_attr(nightly, feature(naked_functions))]
#![cfg_attr(nightly, feature(thread_local))]
#![cfg_attr(test, deny(warnings))]
#![deny(missing_docs)]

#[macro_use]
extern crate log;

mod alloc;
mod detail;
mod gen_impl;
mod reg_context;
mod rt;
mod scope;
mod stack;
mod yield_;

pub use gen_impl::{Generator, GeneratorImpl, Gn};
pub use rt::{get_local_data, is_generator, Error};
pub use scope::Scope;
pub use yield_::{co_get_yield, co_yield_with, done, get_yield, yield_, yield_from, yield_with};
