//! Derive macros for Vela host embedding metadata.

mod attrs;
mod hash;
mod script_host;

use proc_macro::TokenStream;

#[proc_macro_derive(ScriptHost, attributes(script))]
pub fn derive_script_host(input: TokenStream) -> TokenStream {
    script_host::expand(input.into(), script_host::GeneratedMethod::Host).into()
}

#[proc_macro_derive(ScriptReflect, attributes(script))]
pub fn derive_script_reflect(input: TokenStream) -> TokenStream {
    script_host::expand(input.into(), script_host::GeneratedMethod::Reflect).into()
}
