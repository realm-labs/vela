#![cfg_attr(not(test), deny(clippy::wildcard_imports))]

//! Macros for Vela's synchronous, script-owned value boundary.

mod attrs;
mod export;
mod export_module;
mod external_value_enum;
mod hash;
mod signature;
mod type_identity;
mod value;

use proc_macro::TokenStream;

/// Describes and registers one synchronous, value-only Rust function.
#[proc_macro_attribute]
pub fn export(attr: TokenStream, input: TokenStream) -> TokenStream {
    export::expand(attr.into(), input.into()).into()
}

/// Exports the supported public functions in one explicit inline module.
#[proc_macro_attribute]
pub fn export_module(attr: TokenStream, input: TokenStream) -> TokenStream {
    export_module::expand(attr.into(), input.into()).into()
}

/// Generates a centralized structural Value binding for an existing unit enum.
#[proc_macro]
pub fn external_value_enum(input: TokenStream) -> TokenStream {
    external_value_enum::expand(input.into()).into()
}

/// Generates a structural Value codec, schema, and unified TypeBinding.
#[proc_macro_derive(Value, attributes(vela))]
pub fn derive_value(input: TokenStream) -> TokenStream {
    value::expand(input.into()).into()
}
