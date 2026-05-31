mod emission;
mod meta;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemImpl, Result, parse2};

use crate::attrs::{error, spanned_error};
use crate::signature::reject_generic_signature;

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    match expand_result(input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

pub(crate) fn expand_standalone_method(input: TokenStream) -> TokenStream {
    let error = error(
        proc_macro2::Span::call_site(),
        "#[script_method] must be used inside #[script_methods]",
    )
    .to_compile_error();
    quote! {
        #error
        #input
    }
}

fn expand_result(input: TokenStream) -> Result<TokenStream> {
    let mut item = parse2::<ItemImpl>(input)?;
    if item.trait_.is_some() {
        return Err(spanned_error(
            &item,
            "#[script_methods] only supports inherent impl blocks",
        ));
    }
    reject_generic_signature(&item.generics, "#[script_methods]")?;

    let methods = meta::collect_methods(&mut item)?;

    let self_ty = item.self_ty.clone();
    let method_tokens = methods.iter().map(emission::method_tokens);
    let native_registration_tokens = emission::native_method_registration_tokens(&methods);
    let host_method_registration_tokens =
        emission::script_host_method_registration_tokens(&methods);
    Ok(quote! {
        #item

        impl #self_ty {
            #[must_use]
            pub fn vela_native_method_descs() -> ::std::vec::Vec<::vela_engine::NativeMethodDesc> {
                let owner_key = Self::vela_host_type_desc().key;
                let mut methods = ::std::vec::Vec::new();
                #(#method_tokens)*
                methods
            }

            #[must_use]
            pub fn vela_register_native_method_fns(
                builder: ::vela_engine::EngineBuilder,
            ) -> ::vela_engine::EngineBuilder {
                let owner_key = Self::vela_host_type_desc().key;
                #native_registration_tokens
            }

            #[must_use]
            pub fn vela_register_host_methods(
                builder: ::vela_engine::EngineBuilder,
            ) -> ::vela_engine::EngineBuilder {
                let owner_key = Self::vela_host_type_desc().key;
                #host_method_registration_tokens
            }
        }

        impl ::vela_engine::ScriptHostMethodMetadata for #self_ty {
            fn script_host_method_descs() -> ::std::vec::Vec<::vela_engine::NativeMethodDesc> {
                Self::vela_native_method_descs()
            }

            fn register_script_host_methods(
                builder: ::vela_engine::EngineBuilder,
            ) -> ::vela_engine::EngineBuilder {
                Self::vela_register_host_methods(builder)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::expand_result;

    #[test]
    fn rejects_duplicate_method_ids() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn add_exp(player: HostRef, amount: i64) {}

                #[script_method(id = 1)]
                pub fn set_title(player: HostRef, title: String) {}
            }
        })
        .expect_err("duplicate method IDs should fail macro expansion");

        assert!(error.to_string().contains("duplicate script method id"));
    }

    #[test]
    fn rejects_duplicate_method_names() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1, name = "grant")]
                pub fn add_exp(player: HostRef, amount: i64) {}

                #[script_method(id = 2, name = "grant")]
                pub fn grant_exp(player: HostRef, amount: i64) {}
            }
        })
        .expect_err("duplicate method names should fail macro expansion");

        assert!(error.to_string().contains("duplicate script method name"));
    }

    #[test]
    fn rejects_self_receivers() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn add_exp(&mut self, amount: i64) {}
            }
        })
        .expect_err("self receiver should fail macro expansion");

        assert!(error.to_string().contains("HostRef receiver parameters"));
    }

    #[test]
    fn rejects_impl_where_clauses() {
        let error = expand_result(quote! {
            impl Player
            where
                Player: Clone,
            {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef, amount: i64) {}
            }
        })
        .expect_err("impl where clause should fail macro expansion");

        assert!(error.to_string().contains("where clauses"));
    }

    #[test]
    fn rejects_method_where_clauses() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef, amount: i64)
                where
                    i64: Copy,
                {
                }
            }
        })
        .expect_err("method where clause should fail macro expansion");

        assert!(error.to_string().contains("where clauses"));
    }

    #[test]
    fn rejects_unsafe_methods() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub unsafe fn grant(player: HostRef, amount: i64) {
                }
            }
        })
        .expect_err("unsafe method should fail macro expansion");

        assert!(error.to_string().contains("unsafe functions"));
    }

    #[test]
    fn rejects_extern_methods() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub extern "C" fn grant(player: HostRef, amount: i64) {
                }
            }
        })
        .expect_err("extern method should fail macro expansion");

        assert!(error.to_string().contains("extern ABI functions"));
    }

    #[test]
    fn rejects_script_visible_rust_reference_parameters() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef, amount: &mut i64) {}
            }
        })
        .expect_err("script-visible Rust references should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script-visible parameters cannot use Rust references")
        );
    }

    #[test]
    fn rejects_nested_script_visible_rust_reference_parameters() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef, labels: Option<&str>) {}
            }
        })
        .expect_err("nested script-visible Rust references should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script-visible parameters cannot use Rust references")
        );
    }

    #[test]
    fn rejects_script_visible_rust_reference_returns() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn label(player: HostRef) -> &'static str {
                    "gold"
                }
            }
        })
        .expect_err("script-visible Rust reference returns should fail macro expansion");

        assert!(
            error
                .to_string()
                .contains("script-visible returns cannot use Rust references")
        );
    }

    #[test]
    fn rejects_unsupported_integer_parameters() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef, amount: Option<u128>) {}
            }
        })
        .expect_err("unsupported integer parameter should fail macro expansion");

        assert!(error.to_string().contains("u128"));
        assert!(
            error
                .to_string()
                .contains("script-visible native signatures do not support")
        );
    }

    #[test]
    fn rejects_unsupported_integer_parameters_inside_arrays() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef, amounts: [usize; 2]) {}
            }
        })
        .expect_err("unsupported array integer parameter should fail macro expansion");

        assert!(error.to_string().contains("usize"));
    }

    #[test]
    fn rejects_unsupported_integer_returns() {
        let error = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn grant(player: HostRef) -> isize {
                    1
                }
            }
        })
        .expect_err("unsupported integer return should fail macro expansion");

        assert!(error.to_string().contains("isize"));
    }

    #[test]
    fn infers_fixed_array_signature_hints() {
        let tokens = expand_result(quote! {
            impl Player {
                #[script_method(id = 1)]
                pub fn weights(player: HostRef, values: [i64; 3]) -> [i64; 3] {
                    values
                }
            }
        })
        .expect("fixed array method should expand")
        .to_string();

        assert!(tokens.contains("TypeHint :: Array"));
    }
}
