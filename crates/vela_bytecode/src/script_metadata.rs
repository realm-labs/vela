use vela_hir::attributes::derived_traits;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};

use crate::{LinkedProgram, UnlinkedProgramCode};

#[must_use]
pub fn derived_record_trait_fields(
    program: &dyn UnlinkedProgramCode,
    type_name: &str,
    trait_name: &str,
) -> Option<Vec<String>> {
    derived_record_trait_fields_in_graph(program.script_metadata()?, type_name, trait_name)
}

#[must_use]
pub fn derived_linked_record_trait_fields(
    program: &LinkedProgram,
    type_name: &str,
    trait_name: &str,
) -> Option<Vec<String>> {
    derived_record_trait_fields_in_graph(program.script_metadata()?, type_name, trait_name)
}

fn derived_record_trait_fields_in_graph(
    graph: &ModuleGraph,
    type_name: &str,
    trait_name: &str,
) -> Option<Vec<String>> {
    graph.declarations().find_map(|declaration| {
        if declaration.kind != DeclarationKind::Struct {
            return None;
        }
        if graph.qualified_declaration_name(declaration.id).as_deref() != Some(type_name) {
            return None;
        }
        let traits = derived_traits(graph.declaration_attrs(declaration.id));
        if !traits.contains(trait_name) {
            return None;
        }
        let shape = graph.struct_shape(declaration.id)?;
        Some(
            shape
                .fields
                .iter()
                .map(|field| field.name.clone())
                .collect(),
        )
    })
}
