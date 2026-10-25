use std::collections::BTreeSet;

use quote::quote;
use syn::{ItemFn, parse2};

use super::{EffectName, ParameterMode, ReturnMode, TypeShape, classify_function};

fn classify(tokens: proc_macro2::TokenStream) -> super::ClassifiedSignature {
    let item = parse2::<ItemFn>(tokens).expect("test function parses");
    classify_function(&item.sig, &BTreeSet::new()).expect("signature classifies")
}

fn rejection(tokens: proc_macro2::TokenStream) -> String {
    let item = parse2::<ItemFn>(tokens).expect("test function parses");
    match classify_function(&item.sig, &BTreeSet::new()) {
        Ok(_) => panic!("removed signature must fail"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn async_functions_are_rejected_before_boundary_classification() {
    assert!(
        rejection(quote! {
            async fn lookup(table: &Table) -> Option<&Row> { todo!() }
        })
        .contains("Vela exports are synchronous")
    );
}

#[test]
fn owned_and_invocation_local_value_arguments_are_pure() {
    let classified = classify(quote! {
        fn inspect(record: &Record, name: &str, count: i64) -> Option<Record> { todo!() }
    });
    assert_eq!(classified.effects, BTreeSet::from([EffectName::Pure]));
    assert_eq!(
        classified.parameters[0].mode,
        ParameterMode::ReadOnlyValueBorrow
    );
    assert!(matches!(classified.parameters[0].ty, TypeShape::Value(_)));
    assert_eq!(
        classified.parameters[1].mode,
        ParameterMode::ReadOnlyValueBorrow
    );
    assert_eq!(classified.parameters[2].mode, ParameterMode::Value);
    assert_eq!(classified.returns.mode, ReturnMode::Structured);
}

#[test]
fn borrowed_collection_roots_are_rejected() {
    for ty in [
        quote! { &Vec<i64> },
        quote! { &mut Vec<i64> },
        quote! { &BTreeMap<String, i64> },
        quote! { &HashMap<String, i64> },
        quote! { &BTreeSet<i64> },
        quote! { &HashSet<i64> },
        quote! { &[u8] },
        quote! { &mut [u8] },
        quote! { &[u8; 4] },
    ] {
        let message = rejection(quote! { fn inspect(value: #ty) {} });
        assert!(message.contains("borrowed"), "{message}");
    }
}

#[test]
fn borrowed_returns_are_rejected_without_origin_inference() {
    for ty in [
        quote! { &Record },
        quote! { &mut Record },
        quote! { &str },
        quote! { Option<&Record> },
        quote! { Result<&Record, i64> },
        quote! { VmResult<Option<(&Record, &Record)>> },
        quote! { Vec<&Record> },
        quote! { Box<&Record> },
    ] {
        assert!(
            rejection(quote! {
                fn inspect(record: &Record) -> #ty { todo!() }
            })
            .contains("borrowed returns")
        );
    }
}

#[test]
fn host_receivers_mutable_references_and_wrappers_are_rejected() {
    assert!(rejection(quote! { fn inspect(&self) {} }).contains("self receiver"));
    assert!(rejection(quote! { fn inspect(value: &mut Record) {} }).contains("mutable borrowed"));
    for ty in [
        quote! { HostRef },
        quote! { HostPath },
        quote! { PathProxy },
        quote! { HostLeaseRef<Record> },
        quote! { HostLeaseMut<Record> },
        quote! { Host<Record> },
        quote! { View<i64> },
        quote! { MutView<i64> },
        quote! { Option<HostRef> },
        quote! { Vec<HostRef> },
    ] {
        assert!(rejection(quote! { fn inspect(value: #ty) {} }).contains("boundary wrapper"));
    }
}

#[test]
fn explicit_effects_only_add_to_value_calls() {
    let item: ItemFn = syn::parse_quote! { fn notify(value: Record) {} };
    let classified = classify_function(
        &item.sig,
        &BTreeSet::from([EffectName::Random, EffectName::EventEmit]),
    )
    .expect("signature classifies");
    assert_eq!(
        classified.effects,
        BTreeSet::from([EffectName::Random, EffectName::EventEmit])
    );
}
