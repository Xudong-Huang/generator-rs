//! # generator
//!
//! Rust generator library
//!

#![feature(fnbox)]
#![feature(rustc_private)]
#![feature(catch_panic)]
#![feature(box_raw)]
#![cfg_attr(test, deny(warnings))]
#![deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate libc;
extern crate context;

mod rt;
mod generator;
mod fn_gen;
mod yield_;

pub use generator::Generator;
pub use fn_gen::FnGenerator;
pub use yield_::{yield_with, get_yield};
