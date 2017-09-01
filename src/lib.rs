//! # generator
//!
//! Rust generator library
//!

#![cfg_attr(nightly, feature(asm))]
#![cfg_attr(nightly, feature(alloc))]
#![cfg_attr(nightly, feature(fnbox))]
#![cfg_attr(nightly, feature(naked_functions))]
#![cfg_attr(nightly, feature(core_intrinsics))]
#![cfg_attr(nightly, feature(repr_simd))]

#![cfg_attr(test, deny(warnings))]
#![deny(missing_docs)]

#[macro_use]
extern crate log;
#[cfg(nightly)]
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
