use std::collections::BTreeMap;

use vela_def::{FieldId, FunctionId, MethodId, StateId, TypeId, VariantId};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId, HirPatternId};

use crate::MirSourceOrigin;

use super::{CompileGuardKey, MethodExecutableTarget};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct CompileTargetOrigins {
    pub(super) roots: BTreeMap<FunctionId, MirSourceOrigin>,
    pub(super) function_declarations: BTreeMap<HirDeclId, MirSourceOrigin>,
    pub(super) method_targets: BTreeMap<MethodExecutableTarget, MirSourceOrigin>,
    pub(super) lambdas: BTreeMap<(FunctionId, HirBodyId), MirSourceOrigin>,
    pub(super) type_declarations: BTreeMap<HirDeclId, MirSourceOrigin>,
    pub(super) function_descriptors: BTreeMap<FunctionId, MirSourceOrigin>,
    pub(super) method_descriptors: BTreeMap<(TypeId, MethodId), MirSourceOrigin>,
    pub(super) type_descriptors: BTreeMap<TypeId, MirSourceOrigin>,
    pub(super) variant_descriptors: BTreeMap<VariantId, MirSourceOrigin>,
    pub(super) field_descriptors: BTreeMap<FieldId, MirSourceOrigin>,
    pub(super) global_bindings: BTreeMap<HirDeclId, MirSourceOrigin>,
    pub(super) global_descriptors: BTreeMap<StateId, MirSourceOrigin>,
    pub(super) calls: BTreeMap<(FunctionId, HirExprId), MirSourceOrigin>,
    pub(super) members: BTreeMap<(FunctionId, HirExprId), MirSourceOrigin>,
    pub(super) constructors: BTreeMap<(FunctionId, HirExprId), MirSourceOrigin>,
    pub(super) pattern_constructors: BTreeMap<(FunctionId, HirPatternId), MirSourceOrigin>,
    pub(super) host_paths: BTreeMap<(FunctionId, HirExprId), MirSourceOrigin>,
    pub(super) try_targets: BTreeMap<(FunctionId, HirExprId), MirSourceOrigin>,
    pub(super) evaluated_constants: BTreeMap<HirDeclId, MirSourceOrigin>,
    pub(super) evaluated_schema_defaults: BTreeMap<HirBodyId, MirSourceOrigin>,
    pub(super) guards: BTreeMap<CompileGuardKey, MirSourceOrigin>,
}
