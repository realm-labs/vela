use vela_common::{HostMethodId, Span};

use crate::{HostPath, HostValue};

#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub path: HostPath,
    pub op: PatchOp,
    pub source_span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PatchOp {
    Set(HostValue),
    Add(HostValue),
    Sub(HostValue),
    Remove,
    Push(HostValue),
    CallHostMethod {
        method: HostMethodId,
        args: Vec<HostValue>,
    },
}
