use vela_hir::ids::HirBodyId;
use vela_mir::MirSourceOrigin;

use super::param_defaults::ParamDefaultValue;

pub(super) struct FunctionCompileInput {
    pub(super) name: String,
    pub(super) body: HirBodyId,
    pub(super) origin: MirSourceOrigin,
    pub(super) param_defaults: Vec<Option<ParamDefaultValue>>,
}
