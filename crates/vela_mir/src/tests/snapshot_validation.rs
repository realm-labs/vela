use vela_common::{HostTypeId, PrimitiveTag, ShapeId, SourceId, Span};
use vela_def::{FieldId, FunctionId, GlobalId, MethodId, TypeId, VariantId};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirNodeId};

use crate::*;

fn origin(seed: u32) -> MirSourceOrigin {
    MirSourceOrigin::declaration(
        HirDeclId::new(seed),
        Span::new(SourceId::new(40), seed, seed + 5),
    )
}

fn script_function(
    function: FunctionId,
    parameters: Vec<CompileParameter>,
) -> CompileFunctionDescriptor {
    CompileFunctionDescriptor {
        id: function,
        class: CompileFunctionClass::Script,
        canonical_symbol: format!("test::function_{}", function.get()),
        debug_name: format!("function_{}", function.get()),
        signature: CompileSignature {
            parameters,
            positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
            return_contract: None,
            effect: MirEffect::PURE,
        },
        access: CompileFunctionAccess::script(false),
    }
}

fn parameter(name: &str, contract: Option<MirTypeContract>) -> CompileParameter {
    CompileParameter {
        name: name.to_owned(),
        contract,
        default: CompileParameterDefault::Required,
        origin: None,
    }
}

fn assert_input_error(error: MirBuildError, expected_origin: MirSourceOrigin, text: &str) {
    assert_eq!(error.origin(), Some(expected_origin));
    assert!(
        error.to_string().contains(text),
        "expected {error:?} to contain {text:?}"
    );
}

#[test]
fn schema_only_snapshot_finalization_proves_complete_descriptor_closure() {
    let external_type = TypeId::new(400);
    let record_type = TypeId::new(401);
    let record_shape = ShapeId::new(402);
    let record_field = FieldId::new(403);
    let enum_type = TypeId::new(404);
    let variant = VariantId::new(405);
    let variant_field = FieldId::new(406);
    let global = GlobalId::new(407);
    let function = FunctionId::new(408);
    let function_declaration = HirDeclId::new(409);
    let record_declaration = HirDeclId::new(410);
    let enum_declaration = HirDeclId::new(411);
    let global_declaration = HirDeclId::new(412);
    let schema_origin = origin(413);
    let mut builder = CompileTargetSnapshot::builder();

    builder
        .insert_type_descriptor(
            CompileTypeDescriptor {
                id: external_type,
                canonical_name: "host::Opaque".to_owned(),
                class: CompileTypeClass::OpaqueExternal,
                shape: None,
                fields: Vec::new(),
                variants: Vec::new(),
            },
            schema_origin,
        )
        .expect("external type fixture insertion");
    builder
        .insert_script_type(
            record_declaration,
            CompileTypeDescriptor {
                id: record_type,
                canonical_name: "game::Record".to_owned(),
                class: CompileTypeClass::ScriptRecord,
                shape: Some(record_shape),
                fields: vec![record_field],
                variants: Vec::new(),
            },
            schema_origin,
        )
        .expect("record type fixture insertion");
    let record_contract =
        MirTypeContract::Array(Some(Box::new(MirTypeContract::Definition(external_type))));
    builder
        .insert_field_descriptor(
            CompileFieldDescriptor {
                id: record_field,
                owner: record_type,
                variant: None,
                name: "values".to_owned(),
                contract: Some(record_contract.clone()),
                declaration_order: 0,
                access: CompileFieldAccess::script(),
                host_runtime: None,
            },
            schema_origin,
        )
        .expect("record field fixture insertion");
    builder
        .insert_guard(
            CompileGuardKey::Field(record_field),
            CompileGuardTarget {
                contract: record_contract,
                debug_name: "Record::values".to_owned(),
            },
            schema_origin,
        )
        .expect("record field guard fixture insertion");
    builder
        .insert_script_type(
            enum_declaration,
            CompileTypeDescriptor {
                id: enum_type,
                canonical_name: "game::State".to_owned(),
                class: CompileTypeClass::ScriptEnum,
                shape: None,
                fields: Vec::new(),
                variants: vec![variant],
            },
            schema_origin,
        )
        .expect("enum type fixture insertion");
    builder
        .insert_variant_descriptor(
            CompileVariantDescriptor {
                id: variant,
                owner: enum_type,
                name: "Ready".to_owned(),
                fields: vec![variant_field],
                declaration_order: 0,
            },
            schema_origin,
        )
        .expect("variant fixture insertion");
    builder
        .insert_field_descriptor(
            CompileFieldDescriptor {
                id: variant_field,
                owner: enum_type,
                variant: Some(variant),
                name: "count".to_owned(),
                contract: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                declaration_order: 0,
                access: CompileFieldAccess::script(),
                host_runtime: None,
            },
            schema_origin,
        )
        .expect("variant field fixture insertion");
    builder
        .insert_guard(
            CompileGuardKey::Field(variant_field),
            CompileGuardTarget {
                contract: MirTypeContract::Primitive(PrimitiveTag::I64),
                debug_name: "State::Ready::count".to_owned(),
            },
            schema_origin,
        )
        .expect("variant field guard fixture insertion");
    let parameter_contract = MirTypeContract::Definition(record_type);
    let return_contract = MirTypeContract::Variant {
        type_id: enum_type,
        variant,
    };
    let mut descriptor = script_function(
        function,
        vec![parameter("record", Some(parameter_contract.clone()))],
    );
    descriptor.signature.return_contract = Some(return_contract.clone());
    builder
        .insert_script_function_descriptor(function_declaration, descriptor, schema_origin)
        .expect("script function fixture insertion");
    builder
        .insert_guard(
            CompileGuardKey::Parameter {
                function,
                parameter: 0,
            },
            CompileGuardTarget {
                contract: parameter_contract,
                debug_name: "record".to_owned(),
            },
            schema_origin,
        )
        .expect("parameter guard fixture insertion");
    builder
        .insert_guard(
            CompileGuardKey::Return(function),
            CompileGuardTarget {
                contract: return_contract,
                debug_name: "return value".to_owned(),
            },
            schema_origin,
        )
        .expect("return guard fixture insertion");
    let global_contract = MirTypeContract::Definition(record_type);
    builder
        .insert_global(
            global_declaration,
            CompileGlobalDescriptor {
                id: global,
                name: "state".to_owned(),
                contract: global_contract.clone(),
            },
            schema_origin,
        )
        .expect("global fixture insertion");
    builder
        .insert_guard(
            CompileGuardKey::Global(global_declaration),
            CompileGuardTarget {
                contract: global_contract,
                debug_name: "state".to_owned(),
            },
            schema_origin,
        )
        .expect("global guard fixture insertion");
    builder
        .insert_evaluated_schema_default(
            HirBodyId::new(414),
            MirEvaluatedConstant::Scalar(vela_common::ScalarValue::I64(3)),
            schema_origin,
        )
        .expect("schema default fixture insertion");

    let snapshot = builder
        .build()
        .expect("complete schema-only generations must validate without a runtime root");
    assert_eq!(snapshot.compilation_roots().count(), 0);
    assert_eq!(
        snapshot.type_for_declaration(record_declaration),
        Some(record_type)
    );
    assert_eq!(
        snapshot.global(global_declaration).map(|value| value.id),
        Some(global)
    );
}

#[test]
fn finalization_reports_missing_nested_contract_type_at_descriptor_origin() {
    let entry_origin = origin(420);
    let mut builder = CompileTargetSnapshot::builder();
    builder
        .insert_function_descriptor(
            CompileFunctionDescriptor {
                id: FunctionId::new(421),
                class: CompileFunctionClass::Registry,
                canonical_symbol: "host::invalid".to_owned(),
                debug_name: "invalid".to_owned(),
                signature: CompileSignature {
                    parameters: Vec::new(),
                    positional: CompilePositionalPolicy::RuntimeChecked,
                    return_contract: Some(MirTypeContract::Array(Some(Box::new(
                        MirTypeContract::Definition(TypeId::new(422)),
                    )))),
                    effect: MirEffect::external_call(),
                },
                access: CompileFunctionAccess::new(true, true, false),
            },
            entry_origin,
        )
        .expect("invalid function fixture insertion");

    assert_input_error(
        builder
            .build()
            .expect_err("missing type must reject snapshot"),
        entry_origin,
        "missing type",
    );
}

#[test]
fn finalization_rejects_a_placement_without_its_executable_root() {
    let entry_origin = origin(430);
    let mut builder = CompileTargetSnapshot::builder();
    builder
        .insert_call(
            FunctionId::new(431),
            HirExprId::new(432),
            CompileCallTarget::dynamic(CompileCalleeTarget::DynamicCallable, Vec::new()),
            entry_origin,
        )
        .expect("unscoped call fixture insertion");

    assert_input_error(
        builder
            .build()
            .expect_err("unscoped placement must reject snapshot"),
        entry_origin,
        "missing executable root",
    );
}

#[test]
fn finalization_rejects_constructor_defaults_missing_from_the_snapshot() {
    let entry_origin = origin(440);
    let function = FunctionId::new(441);
    let type_id = TypeId::new(442);
    let shape = ShapeId::new(443);
    let field = FieldId::new(444);
    let mut builder = CompileTargetSnapshot::builder();
    builder
        .insert_script_function(
            HirDeclId::new(445),
            HirBodyId::new(446),
            script_function(function, Vec::new()),
            entry_origin,
        )
        .expect("constructor root fixture insertion");
    builder
        .insert_type_descriptor(
            CompileTypeDescriptor {
                id: type_id,
                canonical_name: "game::Config".to_owned(),
                class: CompileTypeClass::ScriptRecord,
                shape: Some(shape),
                fields: vec![field],
                variants: Vec::new(),
            },
            entry_origin,
        )
        .expect("constructor type fixture insertion");
    builder
        .insert_field_descriptor(
            CompileFieldDescriptor {
                id: field,
                owner: type_id,
                variant: None,
                name: "limit".to_owned(),
                contract: None,
                declaration_order: 0,
                access: CompileFieldAccess::script(),
                host_runtime: None,
            },
            entry_origin,
        )
        .expect("constructor field fixture insertion");
    builder
        .insert_constructor(
            function,
            HirExprId::new(447),
            CompileConstructorTarget::Record {
                type_id,
                shape,
                fields: vec![CompileConstructorField {
                    field,
                    parameter: 0,
                    parameter_name: "limit".to_owned(),
                    value: CompileConstructorValue::EvaluatedDefault(HirBodyId::new(448)),
                }],
            },
            entry_origin,
        )
        .expect("constructor placement fixture insertion");

    assert_input_error(
        builder
            .build()
            .expect_err("missing constructor default must reject snapshot"),
        entry_origin,
        "missing evaluated default",
    );
}

#[test]
fn finalization_rejects_unregistered_script_method_executables() {
    let entry_origin = origin(450);
    let owner = TypeId::new(451);
    let executable = MethodExecutableTarget {
        method: MethodId::new(452),
        function: FunctionId::new(453),
        owner,
        node: HirNodeId::new(454),
    };
    let mut builder = CompileTargetSnapshot::builder();
    builder
        .insert_type_descriptor(
            CompileTypeDescriptor {
                id: owner,
                canonical_name: "game::Owner".to_owned(),
                class: CompileTypeClass::ScriptRecord,
                shape: Some(ShapeId::new(455)),
                fields: Vec::new(),
                variants: Vec::new(),
            },
            entry_origin,
        )
        .expect("method owner fixture insertion");
    builder
        .insert_function_descriptor(
            script_function(executable.function, vec![parameter("self", None)]),
            entry_origin,
        )
        .expect("method function fixture insertion");
    builder
        .insert_method_descriptor(
            CompileMethodDescriptor {
                id: executable.method,
                owner,
                member_name: "run".to_owned(),
                debug_name: "Owner::run".to_owned(),
                class: CompileMethodClass::Script {
                    executable,
                    owner_name: "game::Owner".to_owned(),
                    code_symbol: format!("test::function_{}", executable.function.get()),
                },
                signature: CompileSignature {
                    parameters: Vec::new(),
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: None,
                    effect: MirEffect::PURE,
                },
                access: CompileMethodAccess::script(),
            },
            entry_origin,
        )
        .expect("method descriptor fixture insertion");

    assert_input_error(
        builder
            .build()
            .expect_err("unregistered method target must reject snapshot"),
        entry_origin,
        "method-node index",
    );
}

#[test]
fn finalization_rejects_host_runtime_metadata_that_disagrees_at_use_site() {
    let descriptor_origin = origin(460);
    let placement_origin = origin(461);
    let function = FunctionId::new(462);
    let host_type = HostTypeTarget {
        semantic: TypeId::new(463),
        runtime: HostTypeId::new(464),
    };
    let field = FieldId::new(465);
    let access = CompileFieldAccess::new(true, false, true, false, Vec::new());
    let mut builder = CompileTargetSnapshot::builder();
    builder
        .insert_script_function(
            HirDeclId::new(466),
            HirBodyId::new(467),
            script_function(function, Vec::new()),
            descriptor_origin,
        )
        .expect("host root fixture insertion");
    builder
        .insert_type_descriptor(
            CompileTypeDescriptor {
                id: host_type.semantic,
                canonical_name: "host::Player".to_owned(),
                class: CompileTypeClass::Host {
                    runtime: host_type.runtime,
                },
                shape: None,
                fields: vec![field],
                variants: Vec::new(),
            },
            descriptor_origin,
        )
        .expect("host type fixture insertion");
    builder
        .insert_field_descriptor(
            CompileFieldDescriptor {
                id: field,
                owner: host_type.semantic,
                variant: None,
                name: "score".to_owned(),
                contract: None,
                declaration_order: 0,
                access: access.clone(),
                host_runtime: Some(FieldId::new(468)),
            },
            descriptor_origin,
        )
        .expect("host field fixture insertion");
    builder
        .insert_member(
            function,
            HirExprId::new(469),
            CompileMemberTarget::HostField(HostFieldTarget {
                owner: host_type,
                semantic: field,
                runtime: FieldId::new(470),
                access,
            }),
            placement_origin,
        )
        .expect("host member fixture insertion");

    assert_input_error(
        builder
            .build()
            .expect_err("mismatched host runtime must reject snapshot"),
        placement_origin,
        "host field",
    );
}

#[test]
fn finalization_rejects_guard_contracts_that_disagree_with_their_parameter() {
    let descriptor_origin = origin(480);
    let guard_origin = origin(481);
    let function = FunctionId::new(482);
    let mut builder = CompileTargetSnapshot::builder();
    builder
        .insert_script_function_descriptor(
            HirDeclId::new(483),
            script_function(
                function,
                vec![parameter(
                    "value",
                    Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                )],
            ),
            descriptor_origin,
        )
        .expect("guard function fixture insertion");
    builder
        .insert_guard(
            CompileGuardKey::Parameter {
                function,
                parameter: 0,
            },
            CompileGuardTarget {
                contract: MirTypeContract::Primitive(PrimitiveTag::String),
                debug_name: "value".to_owned(),
            },
            guard_origin,
        )
        .expect("mismatched guard fixture insertion");

    assert_input_error(
        builder
            .build()
            .expect_err("mismatched guard must reject snapshot"),
        guard_origin,
        "disagrees",
    );
}
