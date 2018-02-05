//! # generator
//!
//! Rust generator library
//!

#![cfg_attr(nightly, feature(asm))]
#![cfg_attr(nightly, feature(alloc))]
#![cfg_attr(nightly, feature(naked_functions))]
#![cfg_attr(nightly, feature(core_intrinsics))]
#![cfg_attr(nightly, feature(repr_simd))]
#![cfg_attr(nightly, feature(thread_local))]
#![cfg_attr(nightly, feature(untagged_unions))]
#![cfg_attr(test, deny(warnings))]
#![deny(missing_docs)]

#[cfg(nightly)]
extern crate alloc;
#[macro_use]
extern crate log;
#[cfg(not(nightly))]
mod alloc;

mod rt;
mod scope;
mod stack;
mod detail;
mod yield_;
mod no_drop;
mod gen_impl;
mod reg_context;

pub use rt::Error;
pub use scope::Scope;
pub use rt::{get_local_data, is_generator};
pub use gen_impl::{Generator, GeneratorImpl, Gn};
pub use yield_::{co_get_yield, co_yield_with, done, get_yield, yield_, yield_from, yield_with};
