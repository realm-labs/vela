//! Controlled reflection metadata and value access.

pub mod access;
pub mod candidates;
mod descriptor_targets;
pub mod error;
mod error_diagnostics;
mod member_records;
pub mod members;
mod metadata;
mod metadata_records;
pub mod modules;
pub mod permissions;
pub mod registry;
mod script_attrs;
pub mod script_types;
pub mod types;
pub mod value;
pub mod value_access;

#[cfg(test)]
mod tests;
