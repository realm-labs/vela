#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Host reference, path, and direct host access model.

#[forbid(unsafe_code)]
pub mod access;
#[forbid(unsafe_code)]
pub mod adapter;
#[allow(unsafe_code)]
mod erased_slice;
#[forbid(unsafe_code)]
pub mod error;
#[forbid(unsafe_code)]
pub mod lease;
#[forbid(unsafe_code)]
pub mod mock;
#[forbid(unsafe_code)]
pub mod object;
#[forbid(unsafe_code)]
pub mod path;
#[forbid(unsafe_code)]
pub mod protocol;
#[forbid(unsafe_code)]
pub mod proxy;
#[forbid(unsafe_code)]
pub mod resolved;
#[forbid(unsafe_code)]
pub mod target;
#[forbid(unsafe_code)]
pub mod value;

pub(crate) use value::{add_values, div_values, mul_values, rem_values, sub_values};

#[cfg(test)]
mod tests;
