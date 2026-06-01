//! Host reference, path, and patch transaction model.

pub mod adapter;
pub mod error;
pub mod mock;
mod overlay;
pub mod patch;
pub mod path;
pub mod proxy;
pub mod tx;
pub mod value;

pub(crate) use value::{add_values, div_values, mul_values, push_value, rem_values, sub_values};

#[cfg(test)]
mod tests;
