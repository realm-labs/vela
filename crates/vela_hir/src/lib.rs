//! HIR and module graph for Vela source files.

mod ids;
mod module_graph;

pub use ids::{HirDeclId, HirExprId, HirNodeId, ModuleId};
pub use module_graph::{
    Declaration, DeclarationIndex, DeclarationKind, Import, ImportResolution, ModuleGraph,
    ModulePath, ModuleSource, ResolvedImport,
};
