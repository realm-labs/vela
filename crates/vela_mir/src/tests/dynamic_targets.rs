use vela_common::{SourceId, Span};
use vela_def::FunctionId;
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirPatternId};

use crate::*;

fn origin(seed: u32) -> MirSourceOrigin {
    MirSourceOrigin::declaration(
        HirDeclId::new(seed),
        Span::new(SourceId::new(52), seed, seed + 1),
    )
}

fn insert_root(
    builder: &mut CompileTargetSnapshotBuilder,
    function: FunctionId,
    declaration: HirDeclId,
    body: HirBodyId,
    root_origin: MirSourceOrigin,
) {
    builder
        .insert_script_function(declaration, body, script_descriptor(function), root_origin)
        .expect("dynamic target root fixture should be unique");
}

fn script_descriptor(function: FunctionId) -> CompileFunctionDescriptor {
    CompileFunctionDescriptor {
        id: function,
        class: CompileFunctionClass::Script,
        canonical_symbol: format!("test::dynamic_target_{}", function.get()),
        debug_name: format!("dynamic_target_{}", function.get()),
        signature: CompileSignature {
            parameters: Vec::new(),
            positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
            return_contract: None,
            effect: MirEffect::PURE,
        },
        access: CompileFunctionAccess::script(false),
    }
}

fn rooted_builder(seed: u32) -> (CompileTargetSnapshotBuilder, FunctionId) {
    let function = FunctionId::new(seed.into());
    let mut builder = CompileTargetSnapshot::builder();
    insert_root(
        &mut builder,
        function,
        HirDeclId::new(seed + 1),
        HirBodyId::new(seed + 2),
        origin(seed + 3),
    );
    (builder, function)
}

fn field(name: &str, value: u32) -> CompileDynamicConstructorField {
    CompileDynamicConstructorField {
        name: name.to_owned(),
        value: HirExprId::new(value),
    }
}

fn assert_input_error(error: MirBuildError, expected_origin: MirSourceOrigin, expected_text: &str) {
    assert_eq!(error.origin(), Some(expected_origin));
    assert!(
        error.to_string().contains(expected_text),
        "expected {error:?} to contain {expected_text:?}"
    );
}

#[test]
fn dynamic_constructor_and_pattern_targets_are_scoped_and_ordered() {
    let first_function = FunctionId::new(900);
    let second_function = FunctionId::new(901);
    let expression = HirExprId::new(902);
    let pattern = HirPatternId::new(903);
    let root_origin = origin(904);
    let placement_origin = origin(905);
    let first_constructor = CompileConstructorTarget::DynamicRecord {
        type_name: "Reward".to_owned(),
        fields: vec![field("amount", 906), field("label", 907)],
    };
    let second_constructor = CompileConstructorTarget::DynamicVariant {
        owner_name: "Reward".to_owned(),
        variant_name: "Granted".to_owned(),
        fields: vec![field("label", 908), field("amount", 909)],
    };
    let first_pattern = CompilePatternConstructorTarget::DynamicVariant {
        owner_name: "Reward".to_owned(),
        variant_name: "Granted".to_owned(),
        fields: vec!["amount".to_owned(), "amount".to_owned()],
    };
    let second_pattern = CompilePatternConstructorTarget::DynamicVariant {
        owner_name: "Reward".to_owned(),
        variant_name: "Granted".to_owned(),
        fields: vec!["1".to_owned(), "0".to_owned()],
    };
    let mut builder = CompileTargetSnapshot::builder();
    insert_root(
        &mut builder,
        first_function,
        HirDeclId::new(910),
        HirBodyId::new(911),
        root_origin,
    );
    insert_root(
        &mut builder,
        second_function,
        HirDeclId::new(912),
        HirBodyId::new(913),
        root_origin,
    );
    builder
        .insert_constructor(
            first_function,
            expression,
            first_constructor.clone(),
            placement_origin,
        )
        .expect("first dynamic constructor should be unique");
    builder
        .insert_constructor(
            second_function,
            expression,
            second_constructor.clone(),
            placement_origin,
        )
        .expect("a second root may reuse the expression identity");
    builder
        .insert_pattern_constructor(
            first_function,
            pattern,
            first_pattern.clone(),
            placement_origin,
        )
        .expect("first dynamic pattern should be unique");
    builder
        .insert_pattern_constructor(
            second_function,
            pattern,
            second_pattern.clone(),
            placement_origin,
        )
        .expect("a second root may reuse the pattern identity");

    let snapshot = builder
        .build()
        .expect("dynamic names require no fabricated descriptor IDs");
    let first = snapshot
        .function_targets(first_function)
        .expect("first root-scoped targets");
    let second = snapshot
        .function_targets(second_function)
        .expect("second root-scoped targets");
    assert_eq!(first.constructor(expression), Some(&first_constructor));
    assert_eq!(second.constructor(expression), Some(&second_constructor));
    assert_eq!(first.pattern_constructor(pattern), Some(&first_pattern));
    assert_eq!(second.pattern_constructor(pattern), Some(&second_pattern));
}

#[test]
fn function_target_views_require_selected_compilation_roots() {
    let selected = FunctionId::new(914);
    let descriptor_only = FunctionId::new(915);
    let mut builder = CompileTargetSnapshot::builder();
    insert_root(
        &mut builder,
        selected,
        HirDeclId::new(916),
        HirBodyId::new(917),
        origin(918),
    );
    builder
        .insert_script_function_descriptor(
            HirDeclId::new(919),
            script_descriptor(descriptor_only),
            origin(920),
        )
        .expect("unselected script descriptor should be unique");
    let snapshot = builder
        .build()
        .expect("descriptor-only functions need not be selected roots");

    assert!(snapshot.function_targets(selected).is_some());
    assert!(snapshot.function_descriptor(descriptor_only).is_some());
    assert!(snapshot.function_targets(descriptor_only).is_none());
    assert!(snapshot.function_targets(FunctionId::new(921)).is_none());
}

#[test]
fn duplicate_dynamic_placements_use_existing_scoped_errors() {
    let function = FunctionId::new(920);
    let expression = HirExprId::new(921);
    let pattern = HirPatternId::new(922);
    let first_origin = origin(923);
    let duplicate_origin = origin(924);
    let constructor = CompileConstructorTarget::DynamicRecord {
        type_name: "Reward".to_owned(),
        fields: Vec::new(),
    };
    let pattern_target = CompilePatternConstructorTarget::DynamicVariant {
        owner_name: "Reward".to_owned(),
        variant_name: "Granted".to_owned(),
        fields: Vec::new(),
    };
    let mut builder = CompileTargetSnapshot::builder();
    builder
        .insert_constructor(function, expression, constructor.clone(), first_origin)
        .expect("first dynamic constructor placement should be inserted");
    let constructor_error = builder
        .insert_constructor(function, expression, constructor, duplicate_origin)
        .expect_err("duplicate dynamic constructor placement must fail");
    assert!(matches!(
        constructor_error,
        MirBuildError::DuplicateCompileTarget {
            function: duplicate_function,
            kind: CompileTargetKind::Constructor,
            expression: duplicate_expression,
            origin,
        } if duplicate_function == function
            && duplicate_expression == expression
            && origin == duplicate_origin
    ));

    builder
        .insert_pattern_constructor(function, pattern, pattern_target.clone(), first_origin)
        .expect("first dynamic pattern placement should be inserted");
    let pattern_error = builder
        .insert_pattern_constructor(function, pattern, pattern_target, duplicate_origin)
        .expect_err("duplicate dynamic pattern placement must fail");
    assert!(matches!(
        pattern_error,
        MirBuildError::DuplicatePatternConstructor {
            function: duplicate_function,
            pattern: duplicate_pattern,
            origin,
        } if duplicate_function == function
            && duplicate_pattern == pattern
            && origin == duplicate_origin
    ));
}

#[test]
fn dynamic_targets_require_executable_roots_and_retain_origins() {
    let function = FunctionId::new(930);
    let constructor_origin = origin(931);
    let mut constructor_builder = CompileTargetSnapshot::builder();
    constructor_builder
        .insert_constructor(
            function,
            HirExprId::new(932),
            CompileConstructorTarget::DynamicRecord {
                type_name: "Reward".to_owned(),
                fields: Vec::new(),
            },
            constructor_origin,
        )
        .expect("constructor placement fixture should be unique");
    assert_input_error(
        constructor_builder
            .build()
            .expect_err("dynamic constructor without a root must fail"),
        constructor_origin,
        "missing executable root",
    );

    let pattern_origin = origin(933);
    let mut pattern_builder = CompileTargetSnapshot::builder();
    pattern_builder
        .insert_pattern_constructor(
            function,
            HirPatternId::new(934),
            CompilePatternConstructorTarget::DynamicVariant {
                owner_name: "Reward".to_owned(),
                variant_name: "Granted".to_owned(),
                fields: Vec::new(),
            },
            pattern_origin,
        )
        .expect("pattern placement fixture should be unique");
    assert_input_error(
        pattern_builder
            .build()
            .expect_err("dynamic pattern without a root must fail"),
        pattern_origin,
        "missing executable root",
    );
}

#[test]
fn malformed_dynamic_constructor_names_fail_snapshot_closure() {
    let cases = vec![
        (
            CompileConstructorTarget::DynamicRecord {
                type_name: String::new(),
                fields: Vec::new(),
            },
            "empty owner/type name",
        ),
        (
            CompileConstructorTarget::DynamicVariant {
                owner_name: String::new(),
                variant_name: "Granted".to_owned(),
                fields: Vec::new(),
            },
            "empty owner/type name",
        ),
        (
            CompileConstructorTarget::DynamicVariant {
                owner_name: "Reward".to_owned(),
                variant_name: String::new(),
                fields: Vec::new(),
            },
            "empty variant name",
        ),
        (
            CompileConstructorTarget::DynamicRecord {
                type_name: "Reward".to_owned(),
                fields: vec![field("", 950)],
            },
            "empty field name",
        ),
        (
            CompileConstructorTarget::DynamicRecord {
                type_name: "Reward".to_owned(),
                fields: vec![field("amount", 951), field("amount", 952)],
            },
            "duplicate field name",
        ),
    ];
    for (index, (target, expected_text)) in cases.into_iter().enumerate() {
        let seed = 960 + u32::try_from(index).expect("small fixture index") * 10;
        let (mut builder, function) = rooted_builder(seed);
        let target_origin = origin(seed + 4);
        builder
            .insert_constructor(function, HirExprId::new(seed + 5), target, target_origin)
            .expect("malformed constructor fixture should be unique");
        assert_input_error(
            builder
                .build()
                .expect_err("malformed dynamic constructor must fail closure"),
            target_origin,
            expected_text,
        );
    }
}

#[test]
fn malformed_dynamic_pattern_names_fail_without_a_uniqueness_rule() {
    let cases = vec![
        (
            CompilePatternConstructorTarget::DynamicVariant {
                owner_name: String::new(),
                variant_name: "Granted".to_owned(),
                fields: Vec::new(),
            },
            "empty owner/type name",
        ),
        (
            CompilePatternConstructorTarget::DynamicVariant {
                owner_name: "Reward".to_owned(),
                variant_name: String::new(),
                fields: Vec::new(),
            },
            "empty variant name",
        ),
        (
            CompilePatternConstructorTarget::DynamicVariant {
                owner_name: "Reward".to_owned(),
                variant_name: "Granted".to_owned(),
                fields: vec![String::new()],
            },
            "empty field name",
        ),
    ];
    for (index, (target, expected_text)) in cases.into_iter().enumerate() {
        let seed = 1_020 + u32::try_from(index).expect("small fixture index") * 10;
        let (mut builder, function) = rooted_builder(seed);
        let target_origin = origin(seed + 4);
        builder
            .insert_pattern_constructor(
                function,
                HirPatternId::new(seed + 5),
                target,
                target_origin,
            )
            .expect("malformed pattern fixture should be unique");
        assert_input_error(
            builder
                .build()
                .expect_err("malformed dynamic pattern must fail closure"),
            target_origin,
            expected_text,
        );
    }
}
