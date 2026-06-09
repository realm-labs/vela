//! Host reference, path, and direct host access model.

pub mod access;
pub mod adapter;
pub mod error;
pub mod mock;
pub mod object;
pub mod path;
pub mod proxy;
pub mod resolved;
pub mod target;
pub mod value;

pub(crate) use value::{add_values, div_values, mul_values, rem_values, sub_values};

#[cfg(test)]
mod tests;
