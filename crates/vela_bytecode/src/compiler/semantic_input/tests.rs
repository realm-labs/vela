mod external_descriptors;
mod logical_records;
mod roots_schema;
mod target_placements;

use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_hir::body::{HirBodyOwner, HirExprKind, HirPatternKind};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirPatternId};
use vela_registry::RegistryCompileView;

use super::{PreparedSemanticInput, SemanticInputRequest, SemanticRoots, prepare_semantic_input};
use crate::compiler::error::CompileResult;
use crate::compiler::options::CompilerOptions;
use crate::compiler::semantic::parse_semantic_source;

pub(super) enum FixtureRoots<'a> {
    Program,
    Function(&'a str),
}

#[derive(Debug)]
pub(super) struct SemanticFixture {
    pub(super) input: PreparedSemanticInput,
    pub(super) declarations: BTreeMap<String, HirDeclId>,
    pub(super) schema_default_bodies: Vec<HirBodyId>,
    pub(super) try_expressions: Vec<(HirBodyId, HirExprId)>,
    pub(super) member_expressions: Vec<(HirBodyId, HirExprId, String)>,
    pub(super) constructor_expressions: Vec<(HirBodyId, HirExprId, Vec<String>)>,
    pub(super) constructor_patterns: Vec<(HirBodyId, HirPatternId, Vec<String>)>,
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
    let semantic = parse_semantic_source(SourceId::new(900), text)?;
    let roots = match roots {
        FixtureRoots::Program => SemanticRoots::Program,
        FixtureRoots::Function(name) => SemanticRoots::Function(
            semantic
                .function_declaration(name)
                .expect("fixture function must exist"),
        ),
    };
    let mut declarations = BTreeMap::new();
    for declaration in semantic.script_metadata_graph().declarations() {
        // `parse_semantic_source` installs one source at `ModulePath::root()`.
        // The graph's canonical qualified query therefore returns the bare
        // declaration name; there is no synthetic `main::` module segment.
        let name = semantic
            .script_metadata_graph()
            .qualified_declaration_name(declaration.id)
            .expect("fixture declaration must have a canonical graph name");
        debug_assert_eq!(name, declaration.name);
        declarations.insert(name, declaration.id);
    }
    let schema_default_bodies = semantic
        .script_metadata_graph()
        .bodies()
        .filter_map(|body| {
            matches!(body.owner, HirBodyOwner::SchemaFieldDefault(_)).then_some(body.id)
        })
        .collect();
    let try_expressions = semantic
        .script_metadata_graph()
        .bodies()
        .flat_map(|body| {
            body.expressions.values().filter_map(|expression| {
                matches!(&expression.kind, HirExprKind::Try { .. })
                    .then_some((body.id, expression.id))
            })
        })
        .collect();
    let member_expressions = semantic
        .script_metadata_graph()
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
        .script_metadata_graph()
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
        .script_metadata_graph()
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
    let script_function_symbols = semantic.script_function_symbols();
    let script_methods = semantic.script_impl_methods();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let constants = semantic.const_values()?;
    let schema_defaults = semantic.schema_defaults(&type_symbols, &constants)?;
    let options = CompilerOptions::default();
    let input = prepare_semantic_input(SemanticInputRequest {
        graph: semantic.script_metadata_graph(),
        roots,
        script_function_symbols: &script_function_symbols,
        script_methods: &script_methods,
        type_symbols: &type_symbols,
        global_symbols: &global_symbols,
        constants: &constants,
        schema_defaults: &schema_defaults,
        options: &options,
        registry,
    })?;
    Ok(SemanticFixture {
        input,
        declarations,
        schema_default_bodies,
        try_expressions,
        member_expressions,
        constructor_expressions,
        constructor_patterns,
    })
}
