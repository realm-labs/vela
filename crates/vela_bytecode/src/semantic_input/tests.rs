mod external_descriptors;
mod field_assignment_contracts;
mod lambda_targets;
mod literal_ownership;
mod logical_records;
mod roots_schema;
mod script_methods;
mod target_placements;

use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_hir::body::{HirBodyOwner, HirExprKind, HirPatternKind};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirPatternId};
use vela_hir::module_graph::{ModulePath, ModuleSource};
use vela_hir::source_ingestion::build_source_set;
use vela_registry::RegistryCompileView;

use super::{PreparedSemanticInput, SemanticInputRequest, SemanticRoots, prepare_semantic_input};
use crate::compiler::ProgramCompilationMode;
use crate::compiler::error::CompileResult;
use crate::compiler::options::CompilerOptions;
use crate::compiler::semantic::SemanticCompilation;

pub(super) enum FixtureRoots<'a> {
    Program,
    Function(&'a str),
}

#[derive(Debug)]
pub(super) struct SemanticFixture {
    pub(super) input: PreparedSemanticInput,
    pub(super) declarations: BTreeMap<String, HirDeclId>,
    pub(super) schema_default_bodies: Vec<HirBodyId>,
    pub(super) call_expressions: Vec<(HirBodyId, HirExprId)>,
    pub(super) try_expressions: Vec<(HirBodyId, HirExprId)>,
    pub(super) member_expressions: Vec<(HirBodyId, HirExprId, String)>,
    pub(super) constructor_expressions: Vec<(HirBodyId, HirExprId, Vec<String>)>,
    pub(super) constructor_patterns: Vec<(HirBodyId, HirPatternId, Vec<String>)>,
    pub(super) expression_sources: Vec<(HirBodyId, HirExprId, String)>,
}

pub(super) fn prepare_source(
    text: &str,
    roots: FixtureRoots<'_>,
) -> CompileResult<SemanticFixture> {
    prepare_source_inner(text, roots, None)
}

pub(super) fn prepare_source_with_registry(
    text: &str,
    roots: FixtureRoots<'_>,
    registry: RegistryCompileView<'_>,
) -> CompileResult<SemanticFixture> {
    prepare_source_inner(text, roots, Some(registry))
}

fn prepare_source_inner(
    text: &str,
    roots: FixtureRoots<'_>,
    registry: Option<RegistryCompileView<'_>>,
) -> CompileResult<SemanticFixture> {
    let sources = [ModuleSource::new(
        SourceId::new(900),
        ModulePath::new(Vec::<String>::new()),
        text,
    )];
    let built = build_source_set(&sources).expect("semantic fixture source");
    let module = built.modules()[0];
    let mode = ProgramCompilationMode::SingleSource { root: module };
    let semantic = SemanticCompilation::new(built.graph(), &mode)?;
    let roots = match roots {
        FixtureRoots::Program => SemanticRoots::Program,
        FixtureRoots::Function(name) => SemanticRoots::Function(
            built
                .graph()
                .module(module)
                .and_then(|declarations| declarations.get(name))
                .expect("fixture function must exist"),
        ),
    };
    let mut declarations = BTreeMap::new();
    for declaration in semantic.graph().declarations() {
        // Single-source ingestion installs one source at the empty root path.
        // The graph's canonical qualified query therefore returns the bare
        // declaration name; there is no synthetic `main::` module segment.
        let name = semantic
            .graph()
            .qualified_declaration_name(declaration.id)
            .expect("fixture declaration must have a canonical graph name");
        debug_assert_eq!(name, declaration.name);
        declarations.insert(name, declaration.id);
    }
    let schema_default_bodies = semantic
        .graph()
        .bodies()
        .filter_map(|body| {
            matches!(body.owner, HirBodyOwner::SchemaFieldDefault(_)).then_some(body.id)
        })
        .collect();
    let call_expressions = semantic
        .graph()
        .bodies()
        .flat_map(|body| {
            body.calls()
                .map(|(expression, _)| (body.id, expression))
                .collect::<Vec<_>>()
        })
        .collect();
    let try_expressions = semantic
        .graph()
        .bodies()
        .flat_map(|body| {
            body.expressions.values().filter_map(|expression| {
                matches!(&expression.kind, HirExprKind::Try { .. })
                    .then_some((body.id, expression.id))
            })
        })
        .collect();
    let member_expressions = semantic
        .graph()
        .bodies()
        .flat_map(|body| {
            body.expressions.values().filter_map(|expression| {
                let HirExprKind::Field(field) = &expression.kind else {
                    return None;
                };
                Some((body.id, expression.id, field.name.clone()))
            })
        })
        .collect();
    let constructor_expressions = semantic
        .graph()
        .bodies()
        .flat_map(|body| {
            body.expressions.values().filter_map(|expression| {
                let HirExprKind::Record {
                    constructor: Some(path),
                    ..
                } = &expression.kind
                else {
                    return None;
                };
                Some((body.id, expression.id, body.paths.get(path)?.path.clone()))
            })
        })
        .collect();
    let constructor_patterns = semantic
        .graph()
        .bodies()
        .flat_map(|body| {
            body.patterns.values().filter_map(|pattern| {
                let path = match &pattern.kind {
                    HirPatternKind::Path { path }
                    | HirPatternKind::TupleVariant { path, .. }
                    | HirPatternKind::RecordVariant { path, .. } => (*path)?,
                    HirPatternKind::Binding { .. }
                    | HirPatternKind::Wildcard
                    | HirPatternKind::Literal(_)
                    | HirPatternKind::Missing => return None,
                };
                Some((body.id, pattern.id, body.paths.get(&path)?.path.clone()))
            })
        })
        .collect();
    let expression_sources = semantic
        .graph()
        .bodies()
        .flat_map(|body| {
            body.expressions.values().filter_map(|expression| {
                let span = expression.origin.span;
                text.get(span.start as usize..span.end as usize)
                    .map(|source| (body.id, expression.id, source.to_owned()))
            })
        })
        .collect();
    let script_function_symbols = semantic.function_symbols();
    let script_methods = semantic.script_method_catalog();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let evaluated_constants = semantic.evaluated_constants()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &evaluated_constants)?;
    let options = CompilerOptions::default();
    let input = prepare_semantic_input(SemanticInputRequest {
        graph: semantic.graph(),
        roots,
        script_function_symbols: &script_function_symbols,
        script_methods,
        type_symbols: &type_symbols,
        global_symbols: &global_symbols,
        evaluated_constants: &evaluated_constants,
        schema_defaults: &schema_defaults,
        options: &options,
        registry,
    })?;
    Ok(SemanticFixture {
        input,
        declarations,
        schema_default_bodies,
        call_expressions,
        try_expressions,
        member_expressions,
        constructor_expressions,
        constructor_patterns,
        expression_sources,
    })
}
