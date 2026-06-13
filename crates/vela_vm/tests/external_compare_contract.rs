#[path = "../benches/external_compare/version.rs"]
mod version;
#[path = "../benches/external_compare/workloads.rs"]
mod workloads;

use std::collections::BTreeSet;

use mlua::{Function, Lua};
use rhai::{Engine, Scope};
use vela_bytecode::compiler::compile_program_source_with_registry;
use vela_bytecode::{Linker, UnlinkedProgram};
use vela_common::SourceId;
use vela_vm::Vm;

#[test]
fn external_compare_workload_names_are_unique() {
    let mut names = BTreeSet::new();
    for workload in workloads::WORKLOADS {
        assert!(
            names.insert(workload.name),
            "duplicate external compare workload name: {}",
            workload.name
        );
    }
}

#[test]
fn external_compare_workloads_have_all_runtime_sources() {
    for workload in workloads::WORKLOADS {
        assert!(
            !workload.vela.trim().is_empty(),
            "{} missing vela",
            workload.name
        );
        assert!(
            !workload.lua.trim().is_empty(),
            "{} missing lua",
            workload.name
        );
        assert!(
            !workload.rhai.trim().is_empty(),
            "{} missing rhai",
            workload.name
        );
        assert!(
            !workload.node.trim().is_empty(),
            "{} missing node",
            workload.name
        );
        assert!(
            !workload.python.trim().is_empty(),
            "{} missing python",
            workload.name
        );
    }
}

#[test]
fn external_compare_vela_sources_compile_and_link() {
    let vm = Vm::new().with_standard_natives();
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    for workload in workloads::WORKLOADS {
        let program = compile_program_source_with_registry(
            SourceId::new(1),
            workload.vela,
            registry.compile_view(),
        )
        .unwrap_or_else(|error| panic!("{} should compile: {error:?}", workload.name));
        link_program_for_vm(&vm, &program)
            .unwrap_or_else(|error| panic!("{} should link: {error}", workload.name));
    }
}

#[test]
fn embedded_lua_sources_load_and_run() {
    for workload in workloads::WORKLOADS {
        let lua = Lua::new();
        lua.load(workload.lua)
            .exec()
            .unwrap_or_else(|error| panic!("{} lua should load: {error}", workload.name));
        let run = lua
            .globals()
            .get::<Function>("run")
            .unwrap_or_else(|error| panic!("{} lua run function missing: {error}", workload.name));
        let _checksum: i64 = run
            .call(2_i64)
            .unwrap_or_else(|error| panic!("{} lua should run: {error}", workload.name));
    }
}

#[test]
fn embedded_rhai_sources_compile_and_run() {
    let engine = Engine::new();
    for workload in workloads::WORKLOADS {
        let ast = engine
            .compile(workload.rhai)
            .unwrap_or_else(|error| panic!("{} rhai should compile: {error}", workload.name));
        let mut scope = Scope::new();
        let _checksum = engine
            .call_fn::<i64>(&mut scope, &ast, "run", (2_i64,))
            .unwrap_or_else(|error| panic!("{} rhai should run: {error}", workload.name));
    }
}

#[test]
fn python_version_parser_accepts_only_python3_versions() {
    assert_eq!(
        version::python_major_from_version_text("Python 3.14.5"),
        Some(3)
    );
    assert_eq!(
        version::python_major_from_version_text("Python 2.7.18"),
        Some(2)
    );
    assert_eq!(version::python_major_from_version_text("not python"), None);
}

fn link_program_for_vm(
    vm: &Vm,
    program: &UnlinkedProgram,
) -> Result<vela_bytecode::LinkedProgram, Box<dyn std::error::Error>> {
    let mut linker = Linker::new();
    for id in vm.native_implementation_ids() {
        linker.add_native_implementation(id);
    }
    linker
        .link_program(program)
        .map_err(|error| format!("{error:?}").into())
}
