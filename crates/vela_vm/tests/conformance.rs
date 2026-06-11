use vela_bytecode::{
    LinkedProgram, Linker,
    compiler::{
        compile_module_sources_with_registry, compile_program_source_with_registry,
        error::CompileErrorKind,
    },
};
use vela_common::SourceId;
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_vm::Vm;
use vela_vm::error::{VmError, VmErrorKind};
use vela_vm::owned_value::OwnedValue;

const PRIMITIVES_POSITIVE: &str =
    include_str!("../../../tests/fixtures/conformance/primitives_positive.vela");
const PRIMITIVE_SUFFIX_MISMATCH_COMPILE: &str =
    include_str!("../../../tests/fixtures/conformance/primitive_suffix_mismatch_compile.vela");
const PRIMITIVE_STRING_I64_COMPILE: &str =
    include_str!("../../../tests/fixtures/conformance/primitive_string_i64_compile.vela");
const PRIMITIVE_DYNAMIC_GUARD_RUNTIME: &str =
    include_str!("../../../tests/fixtures/conformance/primitive_dynamic_guard_runtime.vela");
const PRIMITIVE_FIELD_GUARD_RUNTIME: &str =
    include_str!("../../../tests/fixtures/conformance/primitive_field_guard_runtime.vela");
const PRIMITIVE_BOUND_LITERAL_RUNTIME: &str =
    include_str!("../../../tests/fixtures/conformance/primitive_bound_literal_runtime.vela");
const PRIMITIVE_MIXED_NUMERIC_RUNTIME: &str =
    include_str!("../../../tests/fixtures/conformance/primitive_mixed_numeric_runtime.vela");
const PRIMITIVE_OVERFLOW_RUNTIME: &str =
    include_str!("../../../tests/fixtures/conformance/primitive_overflow_runtime.vela");

#[test]
fn core_language_fixture_executes() {
    let core = include_str!("../../../tests/fixtures/conformance/core_language.vela");
    let reward = include_str!("../../../tests/fixtures/conformance/reward_module.vela");
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program = compile_module_sources_with_registry(
        &[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_qualified("conformance::core"),
                core,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_qualified("conformance::reward"),
                reward,
            ),
        ],
        registry.compile_view(),
    )
    .expect("core language conformance fixture should compile");
    let mut linker = Linker::with_registry(&registry);
    for spec in vela_stdlib::STD_FUNCTIONS {
        linker.add_native_implementation(spec.id());
    }
    let linked = linker
        .link_program(&program)
        .expect("core language conformance fixture should link");
    let mut vm = Vm::new();
    vm.register_standard_natives();

    let result = vm
        .run_linked_program(&linked, "conformance::core::main", &[])
        .expect("core language conformance fixture should run");

    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(609))
    );
}

#[test]
fn primitive_contract_fixture_executes() {
    let result = run_standard_fixture(PRIMITIVES_POSITIVE)
        .expect("primitive conformance fixture should run");

    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(60))
    );
}

#[test]
fn primitive_negative_fixtures_fail_in_expected_phase() {
    for source in [
        PRIMITIVE_SUFFIX_MISMATCH_COMPILE,
        PRIMITIVE_STRING_I64_COMPILE,
    ] {
        let error = compile_standard_fixture(source)
            .expect_err("static primitive contract mismatch should be a compile error");
        assert!(
            semantic_diagnostics_have_code(&error.kind, "compiler::type_contract_mismatch"),
            "expected type contract mismatch diagnostic, got {error:?}"
        );
    }

    for (source, expected) in [
        (
            PRIMITIVE_DYNAMIC_GUARD_RUNTIME,
            VmErrorKind::TypeContractViolation {
                expected: "i64".to_owned(),
                actual: "string".to_owned(),
                debug_name: "value".to_owned(),
            },
        ),
        (
            PRIMITIVE_FIELD_GUARD_RUNTIME,
            VmErrorKind::TypeContractViolation {
                expected: "i64".to_owned(),
                actual: "string".to_owned(),
                debug_name: "value".to_owned(),
            },
        ),
        (
            PRIMITIVE_BOUND_LITERAL_RUNTIME,
            VmErrorKind::TypeMismatch { operation: "add" },
        ),
        (
            PRIMITIVE_MIXED_NUMERIC_RUNTIME,
            VmErrorKind::TypeMismatch { operation: "add" },
        ),
        (
            PRIMITIVE_OVERFLOW_RUNTIME,
            VmErrorKind::ArithmeticOverflow { operation: "add" },
        ),
    ] {
        let error = run_standard_fixture(source)
            .expect_err("dynamic primitive conformance fixture should fail at runtime");
        assert_eq!(error.kind(), expected);
    }
}

fn compile_standard_fixture(
    source: &str,
) -> vela_bytecode::compiler::error::CompileResult<vela_bytecode::UnlinkedProgram> {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    compile_program_source_with_registry(SourceId::new(10), source, registry.compile_view())
}

fn link_standard_fixture(source: &str) -> LinkedProgram {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let program =
        compile_program_source_with_registry(SourceId::new(10), source, registry.compile_view())
            .expect("conformance fixture should compile");
    let mut linker = Linker::with_registry(&registry);
    for spec in vela_stdlib::STD_FUNCTIONS {
        linker.add_native_implementation(spec.id());
    }
    linker
        .link_program(&program)
        .expect("conformance fixture should link")
}

fn run_standard_fixture(source: &str) -> Result<OwnedValue, VmError> {
    let linked = link_standard_fixture(source);
    let mut vm = Vm::new();
    vm.register_standard_natives();
    vm.run_linked_program(&linked, "main", &[])
}

fn semantic_diagnostics_have_code(kind: &CompileErrorKind, code: &str) -> bool {
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = kind else {
        return false;
    };
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code.as_deref() == Some(code))
}
