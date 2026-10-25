#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Stable embedding API for installing schemas and native functions into Vela.
//!
//! Service deployment and Provider selection are not embedding APIs:
//!
//! ```compile_fail
//! use vela_engine::service::Service;
//! ```
//!
//! ```compile_fail
//! use vela_engine::source::ProviderCompileRequest;
//! ```
//!
//! ```compile_fail
//! use vela_engine::runtime::ProviderHandle;
//! ```
//!
//! Execution and native registration are synchronous. Task admission and async
//! call factories are not embedding APIs:
//!
//! ```compile_fail,E0432
//! use vela_engine::task::TaskScope;
//! ```
//!
//! ```compile_fail,E0432
//! use vela_engine::runtime::RuntimeCallFuture;
//! ```
//!
//! ```compile_fail,E0432
//! use vela_engine::native::AsyncNativeFunctionEntry;
//! ```
//!
//! ```compile_fail,E0432
//! use vela_engine::typed::TypedAsyncNativeFunction;
//! ```
//!
//! ```compile_fail,E0599
//! fn removed(runtime: &mut vela_engine::runtime::Runtime) {
//!     let _ = runtime.call_async("main", vela_engine::runtime::CallArgs::new(),
//!         vela_engine::runtime::CallOptions::new(1_000, 1 << 20, 32));
//! }
//! ```
//!
//! ```compile_fail,E0599
//! let _ = vela_engine::engine::Engine::builder().register_async_fn(
//!     vela_engine::native::NativeFunctionDesc::new("removed", vela_def::FunctionId::new(1)),
//!     |_args| Box::pin(async { Ok(vela_vm::owned_value::OwnedValue::Unit) }),
//! );
//! ```
//!
//! ```compile_fail,E0599
//! fn removed(context: &mut vela_engine::context::NativeCallContext<'_>) {
//!     let _ = context.call_async("main", vela_engine::runtime::CallArgs::new());
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn removed<A: vela_engine::binding::BindingAuthority>(
//!     authority: &mut A, callable: &vela_engine::binding::BindingCallable,
//! ) {
//!     let _ = authority.call_async::<i64, _>(callable, ());
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn removed<A: vela_engine::binding::BindingAuthority>(
//!     authority: &mut A, callable: &vela_engine::binding::BindingCallable,
//! ) {
//!     let _ = authority.call_prepared_async::<i64>(callable,
//!         vela_engine::runtime::CallArgs::new());
//! }
//! ```

//! Borrowed-host injection, path adapters, and host-owned constructors are not
//! embedding APIs. Reference conversion through `ToScriptValueRef` makes owned
//! copies instead.
//!
//! ```compile_fail,E0432
//! use vela_engine::args::{IntoHostArg, TypedHostRef, TypedHostMut};
//! ```
//!
//! ```compile_fail,E0432
//! use vela_engine::schema::ScriptHostSchema;
//! ```
//!
//! ```compile_fail,E0432
//! use vela_engine::host_type::HostTypeSpec;
//! ```
//!
//! ```compile_fail,E0599
//! let _ = vela_engine::runtime::CallArgs::new().with_host_ref("value", &42_i64);
//! ```
//!
//! ```compile_fail,E0599
//! let mut value = 42_i64;
//! let _ = vela_engine::runtime::CallArgs::new().with_host_mut("value", &mut value);
//! ```
//!
//! ```compile_fail,E0599
//! let _ = vela_engine::runtime::CallArgs::new().with_host_handle("value", 42_i64);
//! ```
//!
//! ```compile_fail,E0599
//! let _ = vela_engine::runtime::CallArgs::new().with_fallback_adapter(&mut 42_i64);
//! ```
//!
//! ```compile_fail,E0599
//! fn removed(builder: vela_engine::runtime::RuntimeBuilder) {
//!     let _ = builder.bind_extern_state("value", 42_i64);
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn removed<A: vela_engine::binding::BindingAuthority>(authority: &mut A) {
//!     let _ = authority.push_host_ref(&mut vela_engine::runtime::CallArgs::new(), "value", &42_i64);
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn removed(context: &mut vela_engine::context::NativeCallContext<'_>) {
//!     let _ = context.adapter();
//! }
//! ```
//!
//! ```compile_fail,E0599
//! let _ = vela_engine::engine::Engine::builder().register_host_native_fn(
//!     vela_engine::native::NativeFunctionDesc::new("removed", vela_def::FunctionId::new(1)),
//!     |_, _| Ok(vela_vm::owned_value::OwnedValue::Unit));
//! ```
//!
//! ```compile_fail,E0599
//! let _ = vela_engine::engine::Engine::builder().register_scoped_host_fn(
//!     vela_engine::native::NativeFunctionDesc::new("removed", vela_def::FunctionId::new(1)),
//!     |_| Ok(Vec::new()), |_, _| Ok(vela_vm::owned_value::OwnedValue::Unit));
//! ```
//!
//! ```compile_fail,E0599
//! let _ = vela_engine::type_binding::TypeBinding::<i64>::host(
//!     vela_reflect::registry::TypeDesc::new(vela_reflect::registry::TypeKey::new(vela_def::TypeId::new(1), "Removed")));
//! ```

#![allow(clippy::result_large_err)]

extern crate self as vela_engine;

#[doc(hidden)]
pub mod __private {
    pub use vela_host;
}

pub mod args;
pub mod binding;
pub mod builder;
pub mod clock;
mod compiler_options;
mod compiler_registry;
pub mod context;
pub mod engine;
pub mod error;
pub mod interop;
pub mod io;
mod metadata;
pub mod native;
pub mod permission;
pub mod prelude;
pub mod random;
pub mod registration;
pub mod reload;
pub mod runtime;
pub mod schema;
pub mod source;
pub mod standard;
pub mod type_binding;
pub mod type_registration;
pub mod typed;
mod validation;

#[cfg(feature = "serde")]
pub use vela_vm::serde;

#[cfg(test)]
mod tests;
