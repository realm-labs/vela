//! HIR and module graph for Vela source files.

mod binding;
mod ids;
mod module_graph;
mod top_level;
mod type_hint;

pub use binding::{BindingMap, BindingResolution, ExprInfo, LocalBinding, LocalBindingKind};
pub use ids::{HirDeclId, HirExprId, HirLocalId, HirNodeId, ModuleId};
pub use module_graph::{
    Declaration, DeclarationIndex, DeclarationKind, Import, ImportResolution, ModuleGraph,
    ModulePath, ModuleSource, ResolvedImport,
};
pub use type_hint::{
    ConstMetadata, EnumShape, EnumVariantHint, FunctionSignature, HirTypeHint, ImplMetadata,
    ImplMethodMetadata, ParamHint, StructFieldHint, StructShape,
};
