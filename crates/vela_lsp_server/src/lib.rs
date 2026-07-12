#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Native LSP protocol boundary for Vela editor tooling.

#[cfg(test)]
mod architecture_tests;
mod capabilities;
mod completion;
mod config;
mod config_change;
mod global_state;
mod handlers;
mod lifecycle;
mod line_index;
mod lsp;
pub mod main_loop;
mod paths;
mod profile;
mod protocol;
mod reload;
mod rpc;
mod semantic_tokens;
mod task;
mod tracing;
pub mod transport;
mod watching;

pub use crate::config::LaunchConfiguration;
pub(crate) use crate::rpc::ErrorCode;

#[cfg(test)]
mod tests;
