#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Owned structural values shared by reflection and existing value conversion.
//!
//! Caller-borrowed references and transparent host paths are not supported.
//!
//! ```compile_fail,E0432
//! use vela_host::adapter::ScriptStateAdapter;
//! ```
//!
//! ```compile_fail,E0432
//! use vela_host::path::HostRef;
//! ```
//!
//! ```compile_fail,E0432
//! use vela_host::lease::HostLeaseKind;
//! ```
//!
//! ```compile_fail,E0599
//! use vela_host::call_value::HostCallValue;
//! let value = HostCallValue::HostRef(());
//! ```
//!
//! ```compile_fail,E0432
//! use vela_host::object::HostObject;
//! ```
//!
//! ```compile_fail,E0432
//! use vela_host::slot::HostRefSlots;
//! ```
//!
//! ```compile_fail,E0432
//! use vela_host::proxy::PathProxy;
//! ```
//!
//! ```compile_fail,E0599
//! use vela_host::value::HostValue;
//! let value = HostValue::HostRef(());
//! ```

pub mod call_value;
pub mod error;
pub mod value;
