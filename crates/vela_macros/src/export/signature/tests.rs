use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::quote;
use rstest::rstest;
use syn::{ItemFn, parse2};

use super::{EffectName, ParameterMode, ReturnMode, TypeShape, classify_function};

fn classify(tokens: TokenStream) -> super::ClassifiedSignature {
    let item = parse2::<ItemFn>(tokens).expect("test function parses");
    classify_function(&item.sig, &BTreeSet::new()).expect("signature classifies")
}

fn rejection(tokens: TokenStream) -> String {
    let item = parse2::<ItemFn>(tokens).expect("test function parses");
    match classify_function(&item.sig, &BTreeSet::new()) {
        Ok(_) => panic!("removed signature must fail"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn async_functions_are_rejected_before_boundary_classification() {
    assert_eq!(
        rejection(quote! {
            async fn lookup(table: &Table) -> Option<&Row> { todo!() }
        }),
        "Vela exports are synchronous; async Rust functions cannot be exported or registered"
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

#[rstest]
#[case::vec(quote! { &Vec<i64> }, "borrowed collection parameters are removed; pass an owned collection value")]
#[case::mut_vec(quote! { &mut Vec<i64> }, "mutable borrowed parameters are not supported at the Vela value boundary")]
#[case::btree_map(quote! { &BTreeMap<String, i64> }, "borrowed collection parameters are removed; pass an owned collection value")]
#[case::hash_map(quote! { &HashMap<String, i64> }, "borrowed collection parameters are removed; pass an owned collection value")]
#[case::btree_set(quote! { &BTreeSet<i64> }, "borrowed collection parameters are removed; pass an owned collection value")]
#[case::hash_set(quote! { &HashSet<i64> }, "borrowed collection parameters are removed; pass an owned collection value")]
#[case::slice(quote! { &[u8] }, "borrowed collection parameters are removed; pass an owned collection value")]
#[case::mut_slice(quote! { &mut [u8] }, "mutable borrowed parameters are not supported at the Vela value boundary")]
#[case::array(quote! { &[u8; 4] }, "borrowed collection parameters are removed; pass an owned collection value")]
fn borrowed_collection_roots_are_rejected(#[case] ty: TokenStream, #[case] expected: &str) {
    assert_eq!(rejection(quote! { fn inspect(value: #ty) {} }), expected);
}

#[rstest]
#[case::shared(quote! { &Record })]
#[case::mutable(quote! { &mut Record })]
#[case::str(quote! { &str })]
#[case::option(quote! { Option<&Record> })]
#[case::result(quote! { Result<&Record, i64> })]
#[case::vm_result_option_tuple(quote! { VmResult<Option<(&Record, &Record)>> })]
#[case::vec(quote! { Vec<&Record> })]
#[case::boxed(quote! { Box<&Record> })]
fn borrowed_returns_are_rejected_without_origin_inference(#[case] ty: TokenStream) {
    assert_eq!(
        rejection(quote! {
            fn inspect(record: &Record) -> #ty { todo!() }
        }),
        "borrowed returns and nested references are removed; use owned values"
    );
}

#[rstest]
#[case::receiver(
    quote! { fn inspect(&self) {} },
    "#[vela_macros::export] functions cannot use a self receiver"
)]
#[case::mutable_reference(
    quote! { fn inspect(value: &mut Record) {} },
    "mutable borrowed parameters are not supported at the Vela value boundary"
)]
#[case::host_ref(
    quote! { fn inspect(value: HostRef) {} },
    "ordinary exported signatures cannot mention a Vela boundary wrapper"
)]
#[case::host_path(
    quote! { fn inspect(value: HostPath) {} },
    "ordinary exported signatures cannot mention a Vela boundary wrapper"
)]
#[case::path_proxy(
    quote! { fn inspect(value: PathProxy) {} },
    "ordinary exported signatures cannot mention a Vela boundary wrapper"
)]
#[case::host_lease_ref(
    quote! { fn inspect(value: HostLeaseRef<Record>) {} },
    "ordinary exported signatures cannot mention a Vela boundary wrapper"
)]
#[case::host_lease_mut(
    quote! { fn inspect(value: HostLeaseMut<Record>) {} },
    "ordinary exported signatures cannot mention a Vela boundary wrapper"
)]
#[case::host(
    quote! { fn inspect(value: Host<Record>) {} },
    "ordinary exported signatures cannot mention a Vela boundary wrapper"
)]
#[case::view(
    quote! { fn inspect(value: View<i64>) {} },
    "ordinary exported signatures cannot mention a Vela boundary wrapper"
)]
#[case::mut_view(
    quote! { fn inspect(value: MutView<i64>) {} },
    "ordinary exported signatures cannot mention a Vela boundary wrapper"
)]
#[case::optional_host_ref(
    quote! { fn inspect(value: Option<HostRef>) {} },
    "ordinary exported signatures cannot mention a Vela boundary wrapper"
)]
#[case::vec_host_ref(
    quote! { fn inspect(value: Vec<HostRef>) {} },
    "ordinary exported signatures cannot mention a Vela boundary wrapper"
)]
fn host_receivers_mutable_references_and_wrappers_are_rejected(
    #[case] input: TokenStream,
    #[case] expected: &str,
) {
    assert_eq!(rejection(input), expected);
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
