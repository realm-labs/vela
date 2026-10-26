use super::*;

// SF06 owns execution equivalence. This lane only establishes that these
// extracted sources fit the existing codec and link without a host registry.
#[rstest::rstest]
#[case::nested_captures(include_str!("../../../tests/fixtures/compiler/nested_captures.vela"))]
#[case::scalar_loop(include_str!("../../../tests/fixtures/compiler/scalar_loop.vela"))]
#[case::typed_record_write(include_str!("../../../tests/fixtures/compiler/typed_record_write.vela"))]
#[case::named_default_method(include_str!("../../../tests/fixtures/compiler/named_default_method.vela"))]
fn extracted_fixture_is_portable_supported(#[case] source: &str) {
    let program = compile_test_program(SourceId::new(1), source).expect("source compilation");
    let artifact =
        crate::PortableProgramArtifact::from_compiled(program).expect("existing portable format");
    let encoded = artifact.encode().expect("encode");
    let decoded = crate::PortableProgramArtifact::decode(&encoded).expect("decode");
    assert_eq!(decoded, artifact);
    assert_eq!(
        decoded.encode().expect("deterministic re-encoding"),
        encoded
    );
    let linked = crate::Linker::new()
        .link_portable_program(decoded.into_compiled())
        .expect("link decoded artifact");
    linked.verify().expect("verify decoded artifact");
}
