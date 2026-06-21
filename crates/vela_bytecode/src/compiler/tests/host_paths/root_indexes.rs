use super::*;

#[test]
fn compiler_lowers_root_host_index_receiver() {
    let scores_type = HostTypeId::new(88);
    let mut registry = vela_registry::DefinitionRegistry::new();
    register_registry_host_type(&mut registry, "Scores", scores_type);
    let code = compile_function_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn main(scores: Scores, key) {
    scores[key] = 10;
    scores[key] += 2;
    return scores[key];
}
"#,
        "main",
        &CompilerOptions::new().with_host_index_capability(
            "Scores",
            HostIndexCapabilityInfo {
                readable: true,
                writable: true,
                addable: true,
                key_type: Some("i64".to_owned()),
                value_type: Some("i64".to_owned()),
                ..HostIndexCapabilityInfo::default()
            },
        ),
        registry.compile_view(),
    )
    .expect("root host index receiver should compile");

    let Some(target) = code
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            UnlinkedInstructionKind::HostRead { target, .. } => Some(target),
            _ => None,
        })
    else {
        panic!("expected HostRead");
    };
    let plan = code.host_target(target).expect("host target should exist");
    assert_eq!(plan.root_type, scores_type);
    assert_eq!(plan.parts.as_slice(), [HostPathPart::DynIndex { arg: 0 }]);
    assert!(has_host_read_target(
        &code,
        &[HostPathPart::DynIndex { arg: 0 }],
        1
    ));
    assert!(has_host_write_target(
        &code,
        &[HostPathPart::DynIndex { arg: 0 }],
        1
    ));
    assert!(has_host_mutate_target(
        &code,
        vela_host::resolved::HostMutationOp::Add,
        &[HostPathPart::DynIndex { arg: 0 }],
        1
    ));
}

#[test]
fn compiler_lowers_root_host_index_block_keys_through_cst_payloads() {
    let scores_type = HostTypeId::new(88);
    let mut registry = vela_registry::DefinitionRegistry::new();
    register_registry_host_type(&mut registry, "Scores", scores_type);
    let code = compile_function_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn main(scores: Scores) {
    scores[{
        let first = 1;
        first
    }] = 10;
    scores[{
        let second = 2;
        second
    }] += 2;
    scores[{
        let third = 3;
        third
    }].remove();
    return scores[{
        let fourth = 4;
        fourth
    }];
}
"#,
        "main",
        &CompilerOptions::new().with_host_index_capability(
            "Scores",
            HostIndexCapabilityInfo {
                readable: true,
                writable: true,
                addable: true,
                removable: true,
                key_type: Some("i64".to_owned()),
                value_type: Some("i64".to_owned()),
            },
        ),
        registry.compile_view(),
    )
    .expect("block-expression host index keys should compile through CST payloads");

    assert!(has_host_write_target(
        &code,
        &[HostPathPart::DynIndex { arg: 0 }],
        1
    ));
    assert!(has_host_mutate_target(
        &code,
        vela_host::resolved::HostMutationOp::Add,
        &[HostPathPart::DynIndex { arg: 0 }],
        1
    ));
    assert!(has_host_read_target(
        &code,
        &[HostPathPart::DynIndex { arg: 0 }],
        1
    ));
    assert!(
        code.instructions
            .iter()
            .any(|instruction| match &instruction.kind {
                UnlinkedInstructionKind::HostRemove {
                    target,
                    dynamic_args,
                    ..
                } =>
                    dynamic_args.len() == 1
                        && host_target_parts(&code, *target) == [HostPathPart::DynIndex { arg: 0 }],
                _ => false,
            })
    );
}

#[test]
fn compiler_rejects_invalid_root_host_index_accesses() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    register_registry_host_type(&mut registry, "Scores", HostTypeId::new(88));

    let error = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(scores: Scores) {
    return scores[0];
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect_err("unindexed host type should fail");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::host_index_not_supported"]
    );

    let error = compile_function_source_with_options_and_registry(
        SourceId::new(2),
        r#"
fn main(scores: Scores) {
    return scores[0];
}
"#,
        "main",
        &CompilerOptions::new().with_host_index_capability(
            "Scores",
            HostIndexCapabilityInfo {
                readable: false,
                key_type: Some("i64".to_owned()),
                value_type: Some("i64".to_owned()),
                ..HostIndexCapabilityInfo::default()
            },
        ),
        registry.compile_view(),
    )
    .expect_err("non-readable host index should fail");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::host_index_not_readable"]
    );

    let error = compile_function_source_with_options_and_registry(
        SourceId::new(3),
        r#"
fn main(scores: Scores) {
    return scores["bad"];
}
"#,
        "main",
        &CompilerOptions::new().with_host_index_capability(
            "Scores",
            HostIndexCapabilityInfo {
                readable: true,
                key_type: Some("i64".to_owned()),
                value_type: Some("i64".to_owned()),
                ..HostIndexCapabilityInfo::default()
            },
        ),
        registry.compile_view(),
    )
    .expect_err("wrong host index key type should fail");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::host_index_key_mismatch"]
    );

    let error = compile_function_source_with_options_and_registry(
        SourceId::new(4),
        r#"
fn main(scores: Scores) {
    scores[0] = 1;
    return 1;
}
"#,
        "main",
        &CompilerOptions::new().with_host_index_capability(
            "Scores",
            HostIndexCapabilityInfo {
                readable: true,
                writable: false,
                key_type: Some("i64".to_owned()),
                value_type: Some("i64".to_owned()),
                ..HostIndexCapabilityInfo::default()
            },
        ),
        registry.compile_view(),
    )
    .expect_err("non-writable host index should fail");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::host_index_not_writable"]
    );

    let error = compile_function_source_with_options_and_registry(
        SourceId::new(5),
        r#"
fn main(scores: Scores) {
    scores[0] += 1;
    return 1;
}
"#,
        "main",
        &CompilerOptions::new().with_host_index_capability(
            "Scores",
            HostIndexCapabilityInfo {
                readable: true,
                writable: true,
                addable: false,
                key_type: Some("i64".to_owned()),
                value_type: Some("i64".to_owned()),
                ..HostIndexCapabilityInfo::default()
            },
        ),
        registry.compile_view(),
    )
    .expect_err("non-addable host index mutation should fail");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::host_index_not_mutable"]
    );

    let error = compile_function_source_with_options_and_registry(
        SourceId::new(6),
        r#"
fn main(scores: Scores) {
    scores[0].remove();
    return 1;
}
"#,
        "main",
        &CompilerOptions::new().with_host_index_capability(
            "Scores",
            HostIndexCapabilityInfo {
                readable: true,
                writable: true,
                addable: true,
                removable: false,
                key_type: Some("i64".to_owned()),
                value_type: Some("i64".to_owned()),
            },
        ),
        registry.compile_view(),
    )
    .expect_err("non-removable host index should fail");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::host_index_not_removable"]
    );
}

#[test]
fn compiler_lowers_removable_root_host_index_remove_calls() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    register_registry_host_type(&mut registry, "Scores", HostTypeId::new(88));
    let code = compile_function_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn main(scores: Scores, key) {
    scores[key].remove();
    return 1;
}
"#,
        "main",
        &CompilerOptions::new().with_host_index_capability(
            "Scores",
            HostIndexCapabilityInfo {
                readable: true,
                writable: true,
                addable: true,
                removable: true,
                key_type: Some("i64".to_owned()),
                value_type: Some("i64".to_owned()),
            },
        ),
        registry.compile_view(),
    )
    .expect("removable root host index should compile");

    assert!(
        code.instructions
            .iter()
            .any(|instruction| match &instruction.kind {
                UnlinkedInstructionKind::HostRemove {
                    target,
                    dynamic_args,
                    ..
                } =>
                    dynamic_args.len() == 1
                        && host_target_parts(&code, *target) == [HostPathPart::DynIndex { arg: 0 }],
                _ => false,
            })
    );
}
