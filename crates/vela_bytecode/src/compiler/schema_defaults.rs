//! Compile-time schema-default values retained outside runtime MIR.

use std::collections::BTreeMap;

use vela_hir::body::HirBodyOwner;
use vela_hir::ids::{HirBodyId, HirDeclId, ModuleId};
use vela_hir::module_graph::ModuleGraph;
use vela_mir::MirEvaluatedConstant;

use super::const_eval::evaluate_const_body;
use super::error::CompileResult;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct EvaluatedSchemaDefaults {
    evaluated_defaults: BTreeMap<HirBodyId, Option<MirEvaluatedConstant>>,
}

impl EvaluatedSchemaDefaults {
    pub(super) fn merge(&mut self, other: Self) {
        self.evaluated_defaults.extend(other.evaluated_defaults);
    }

    pub(super) fn evaluated_defaults(&self) -> &BTreeMap<HirBodyId, Option<MirEvaluatedConstant>> {
        &self.evaluated_defaults
    }
}

pub(super) fn source_schema_defaults(
    graph: &ModuleGraph,
    _module: ModuleId,
    type_symbols: &BTreeMap<HirDeclId, String>,
    evaluated_constants: &BTreeMap<HirDeclId, MirEvaluatedConstant>,
) -> CompileResult<EvaluatedSchemaDefaults> {
    let mut evaluated_defaults = BTreeMap::new();
    for body in graph.bodies() {
        let HirBodyOwner::SchemaFieldDefault(declaration) = body.owner else {
            continue;
        };
        if !type_symbols.contains_key(&declaration) {
            continue;
        }
        let Some(bindings) = graph.schema_field_default_bindings(body.id) else {
            continue;
        };
        evaluated_defaults.insert(
            body.id,
            evaluate_const_body(body, bindings, evaluated_constants)?,
        );
    }
    Ok(EvaluatedSchemaDefaults { evaluated_defaults })
}
