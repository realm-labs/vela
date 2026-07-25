use std::collections::BTreeSet;

use quote::quote;
use syn::{ItemFn, parse2};

use super::{
    BorrowedCollectionKind, EffectName, HostAccess, ParameterMode, ReturnMode, TypeShape,
    classify_function, classify_method,
};

fn classify(tokens: proc_macro2::TokenStream) -> super::ClassifiedSignature {
    let item = parse2::<ItemFn>(tokens).expect("test function parses");
    classify_function(&item.sig, &BTreeSet::new()).expect("signature classifies")
}

#[test]
fn signature_infers_normalized_effects() {
    let pure = classify(quote! { fn normalize(value: i64) -> i64 { value } });
    let shared = classify(quote! { fn level(player: &Player) -> i64 { 0 } });
    let exclusive = classify(quote! { fn grant(player: &mut Player) {} });

    assert_eq!(pure.effects, BTreeSet::from([EffectName::Pure]));
    assert_eq!(shared.effects, BTreeSet::from([EffectName::HostRead]));
    assert_eq!(exclusive.effects, BTreeSet::from([EffectName::HostWrite]));
    assert_eq!(exclusive.parameters[0].mode, ParameterMode::ExclusiveHost);
}

#[test]
fn standard_collection_references_classify_as_host_backed_views() {
    let classified = classify(quote! {
        fn patch(
            values: &Vec<i64>,
            scores: &mut BTreeMap<String, i64>,
            tags: &HashSet<String>,
        ) {}
    });

    assert!(matches!(
        &classified.parameters[0].ty,
        TypeShape::BorrowedCollection(collection)
            if collection.access == HostAccess::Shared
                && matches!(collection.kind, BorrowedCollectionKind::Array(_))
    ));
    assert!(matches!(
        &classified.parameters[1].ty,
        TypeShape::BorrowedCollection(collection)
            if collection.access == HostAccess::Exclusive
                && matches!(collection.kind, BorrowedCollectionKind::Map(_, _))
    ));
    assert!(matches!(
        &classified.parameters[2].ty,
        TypeShape::BorrowedCollection(collection)
            if collection.access == HostAccess::Shared
                && matches!(collection.kind, BorrowedCollectionKind::Set(_))
    ));
    assert_eq!(classified.effects, BTreeSet::from([EffectName::HostWrite]));
}

#[test]
fn byte_collection_references_classify_as_host_backed_views() {
    let classified = classify(quote! {
        fn patch(shared: &[u8], fixed: &mut [u8; 4], growable: &mut Vec<u8>) {}
    });

    assert!(matches!(
        &classified.parameters[0].ty,
        TypeShape::BorrowedCollection(collection)
            if collection.access == HostAccess::Shared
                && collection.mutation == vela_common::CollectionViewMutation::Fixed
                && matches!(collection.kind, BorrowedCollectionKind::Array(_))
    ));
    assert!(matches!(
        &classified.parameters[1].ty,
        TypeShape::BorrowedCollection(collection)
            if collection.access == HostAccess::Exclusive
                && collection.mutation == vela_common::CollectionViewMutation::Fixed
                && matches!(collection.kind, BorrowedCollectionKind::Array(_))
    ));
    assert!(matches!(
        &classified.parameters[2].ty,
        TypeShape::BorrowedCollection(collection)
            if collection.access == HostAccess::Exclusive
                && collection.mutation == vela_common::CollectionViewMutation::Growable
                && matches!(collection.kind, BorrowedCollectionKind::Array(_))
    ));
}

#[test]
fn explicit_effects_only_add_to_inferred_base() {
    let item = parse2::<ItemFn>(quote! {
        fn notify(player: &mut Player) {}
    })
    .expect("test function parses");
    let classified = classify_function(
        &item.sig,
        &BTreeSet::from([EffectName::Random, EffectName::EventEmit]),
    )
    .expect("signature classifies");

    assert_eq!(
        classified.effects,
        BTreeSet::from([
            EffectName::HostWrite,
            EffectName::Random,
            EffectName::EventEmit,
        ])
    );
}

#[test]
fn borrowed_return_requires_one_owner_origin() {
    let item = parse2::<ItemFn>(quote! {
        fn player(left: &Game, right: &Game) -> &Player { todo!() }
    })
    .expect("test function parses");
    let error = match classify_function(&item.sig, &BTreeSet::new()) {
        Ok(_) => panic!("ambiguous borrowed return must fail"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("exactly one unambiguous"));
}

#[test]
fn method_receiver_infers_effect_and_borrow_origin() {
    let item = parse2::<syn::ImplItemFn>(quote! {
        pub fn player(&self) -> &Player { todo!() }
    })
    .expect("test method parses");
    let classified =
        classify_method(&item.sig, &BTreeSet::new()).expect("method signature classifies");

    assert_eq!(classified.effects, BTreeSet::from([EffectName::HostRead]));
    assert!(matches!(
        classified.returns.mode,
        ReturnMode::ScopedHost {
            origin: super::BorrowOrigin::Receiver,
            ..
        }
    ));
}
