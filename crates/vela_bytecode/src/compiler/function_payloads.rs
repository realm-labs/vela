use super::body_payloads::CompilerBodyPayload;
use super::param_defaults::ParamDefaultValue;

pub(super) struct FunctionBodyPayload<'ast> {
    pub(super) name: String,
    pub(super) body: CompilerBodyPayload<'ast>,
    pub(super) param_defaults: Vec<Option<ParamDefaultValue>>,
}
