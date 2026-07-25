use super::*;
use crate::UnlinkedInstructionKind;
use crate::compiler::service_schema::{
    ServiceCompilationMethod, ServiceCompilationSchema, ServiceCompilationService,
};
use vela_common::{
    CallableAsyncness, ServiceCallMode, ServiceId, ServiceMethodId, ServiceSetId,
    service_dispatch_stable_id,
};
use vela_mir::MirEffect;

const INVENTORY_SERVICE: ServiceId = ServiceId::new(0x11);
const INVENTORY_GRANT: ServiceMethodId = ServiceMethodId::new(0x12);
const AUDIT_SERVICE: ServiceId = ServiceId::new(0x21);
const AUDIT_RECORD: ServiceMethodId = ServiceMethodId::new(0x22);

fn service_schema() -> ServiceCompilationSchema {
    ServiceCompilationSchema::new(
        ServiceSetId::new(0x01),
        [
            ServiceCompilationService::new(
                INVENTORY_SERVICE,
                "inventory",
                "game::inventory::InventoryService",
                [ServiceCompilationMethod::new(
                    INVENTORY_GRANT,
                    "grant",
                    1,
                    CallableAsyncness::Sync,
                    MirEffect::PURE,
                )],
            ),
            ServiceCompilationService::new(
                AUDIT_SERVICE,
                "audit",
                "game::audit::AuditService",
                [ServiceCompilationMethod::new(
                    AUDIT_RECORD,
                    "record",
                    1,
                    CallableAsyncness::Sync,
                    MirEffect::PURE,
                )],
            ),
        ],
    )
}

fn compile_service_program(source: &str) -> Result<CompiledProgram, CompileError> {
    let sources = vela_hir::source_ingestion::build_single_source(SourceId::new(11), source)
        .expect("service test HIR source set");
    compile_program_with_service_schema(
        ProgramCompilationRequest {
            sources: &sources,
            options: &CompilerOptions::default(),
            registry: None,
        },
        &service_schema(),
    )
}

#[test]
fn compiler_emits_service_impl_methods_as_hidden_stable_functions() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value: i64) -> i64 {
        return value * 3;
    }
}
"#,
    )
    .expect("service method should compile");
    let symbol = "__service_impl.game.inventory.InventoryService.grant";
    let function =
        vela_def::script_function_id(vela_package::PackageId::anonymous().as_str(), symbol);
    let code = program
        .function_by_id(function)
        .expect("stable hidden service function");

    assert_eq!(code.name, symbol);
    assert_eq!(code.params, ["value"]);
    assert!(
        program.function("grant").is_none(),
        "service method must not become a public function"
    );
}

#[test]
fn compiler_lowers_base_and_pinned_service_calls_to_stable_dispatch_targets() {
    let program = compile_service_program(
        r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value: i64) -> i64 {
        let default = base.grant(value);
        return services.audit.record(default);
    }
}
"#,
    )
    .expect("lexical service calls should compile");
    let function = vela_def::script_function_id(
        vela_package::PackageId::anonymous().as_str(),
        "__service_impl.game.inventory.InventoryService.grant",
    );
    let code = program
        .function_by_id(function)
        .expect("hidden service implementation");
    let calls = code
        .instructions
        .iter()
        .filter_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallNative {
                name, native, args, ..
            } => Some((name.as_str(), *native, args.len())),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        calls,
        [
            (
                "__vela_service.base.game.inventory.InventoryService.grant",
                FunctionId::new(service_dispatch_stable_id(
                    ServiceCallMode::Base,
                    INVENTORY_SERVICE,
                    INVENTORY_GRANT,
                )),
                1,
            ),
            (
                "__vela_service.pinned.game.audit.AuditService.record",
                FunctionId::new(service_dispatch_stable_id(
                    ServiceCallMode::Pinned,
                    AUDIT_SERVICE,
                    AUDIT_RECORD,
                )),
                1,
            ),
        ]
    );
}

#[test]
fn compiler_rejects_unknown_or_malformed_service_calls() {
    let unknown_service = compile_service_program(
        r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value: i64) -> i64 {
        return services.missing.record(value);
    }
}
"#,
    )
    .expect_err("unknown service member must fail");
    assert_eq!(
        unknown_service.kind,
        CompileErrorKind::ServiceCall("unknown service target `missing`".to_owned())
    );

    let wrong_arity = compile_service_program(
        r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value: i64) -> i64 {
        return base.grant(value, value);
    }
}
"#,
    )
    .expect_err("wrong service arity must fail");
    assert!(matches!(
        wrong_arity.kind,
        CompileErrorKind::ServiceCall(message)
            if message.contains("expects 1 arguments, found 2")
    ));
}

#[test]
fn compiler_requires_a_sealed_schema_for_lexical_service_calls() {
    let error = compile_test_program(
        SourceId::new(12),
        r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value: i64) -> i64 {
        return base.grant(value);
    }
}
"#,
    )
    .expect_err("service calls without a schema must fail");

    assert_eq!(
        error.kind,
        CompileErrorKind::ServiceCall(
            "service calls require the generated service-set schema".to_owned(),
        )
    );
}

#[test]
fn compiler_keeps_service_dispatch_capabilities_lexical_and_non_first_class() {
    for source in [
        r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value: i64) -> i64 {
        let target = services.audit.record;
        return target(value);
    }
}
"#,
        r#"
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(value: i64) -> i64 {
        return reflect::call(services.audit, "record", value);
    }
}
"#,
    ] {
        let error = vela_hir::source_ingestion::build_single_source(SourceId::new(13), source)
            .expect_err("service capabilities must not become dynamic or reflection values");
        assert!(error.diagnostics().iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("hir::invalid_service_capability_use")
                && diagnostic.message.contains("scoped service capability")
        }));
    }
}
