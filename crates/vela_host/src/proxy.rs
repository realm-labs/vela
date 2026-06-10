use vela_common::{HostMethodId, Span};
use vela_def::FieldId;

use crate::{
    access::HostAccess,
    adapter::ScriptStateAdapter,
    error::HostResult,
    path::{HostPath, HostRef},
    resolved::HostMutationOp,
    target::{HostPathArg, HostPathArgOwned, HostPathPart, HostTargetInstance, HostTargetPlan},
    value::HostValue,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathProxy {
    root: HostRef,
    target: Box<HostTargetPlan>,
    args: Vec<HostPathArgOwned>,
}

impl PathProxy {
    #[must_use]
    pub fn new(root: HostRef, target: HostTargetPlan) -> Self {
        Self {
            root,
            target: Box::new(target),
            args: Vec::new(),
        }
    }

    #[must_use]
    pub fn from_diagnostic_path(path: HostPath) -> Self {
        Self::new(path.root, HostTargetPlan::from(&path))
    }

    #[must_use]
    pub fn root(&self) -> HostRef {
        self.root
    }

    #[must_use]
    pub fn target(&self) -> &HostTargetPlan {
        self.target.as_ref()
    }

    #[must_use]
    pub fn args(&self) -> &[HostPathArgOwned] {
        &self.args
    }

    #[must_use]
    pub fn to_diagnostic_path(&self) -> HostPath {
        let args = self.borrowed_args();
        HostTargetInstance::new(self.root, self.target(), &args)
            .to_diagnostic_path()
            .to_host_path()
    }

    #[must_use]
    pub fn field(mut self, field: FieldId) -> Self {
        self.target.parts.push(HostPathPart::Field(field));
        self
    }

    #[must_use]
    pub fn index(mut self, index: u32) -> Self {
        let arg = self.push_arg(HostPathArgOwned::Index(index));
        self.target.parts.push(HostPathPart::DynIndex { arg });
        self
    }

    #[must_use]
    pub fn key(mut self, key: impl Into<String>) -> Self {
        let arg = self.push_arg(HostPathArgOwned::Key(key.into()));
        self.target.parts.push(HostPathPart::DynKey { arg });
        self
    }

    #[must_use]
    pub fn const_index(mut self, index: u32) -> Self {
        self.target.parts.push(HostPathPart::ConstIndex(index));
        self
    }

    #[must_use]
    pub fn const_key(mut self, key: impl Into<String>) -> Self {
        self.target.parts.push(HostPathPart::ConstKey(key.into()));
        self
    }

    #[must_use]
    pub fn variant_field(mut self, field: FieldId) -> Self {
        self.target.parts.push(HostPathPart::VariantField(field));
        self
    }

    pub fn read(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &HostAccess,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        let args = self.borrowed_args();
        let target = HostTargetInstance::new(self.root, self.target(), &args);
        access.read(adapter, target, source_span)
    }

    pub fn set(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let args = self.borrowed_args();
        let target = HostTargetInstance::new(self.root, self.target(), &args);
        access.write(adapter, target, value, source_span)
    }

    pub fn add(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate(adapter, access, HostMutationOp::Add, value, source_span)
    }

    pub fn sub(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate(adapter, access, HostMutationOp::Sub, value, source_span)
    }

    pub fn mul(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate(adapter, access, HostMutationOp::Mul, value, source_span)
    }

    pub fn div(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate(adapter, access, HostMutationOp::Div, value, source_span)
    }

    pub fn rem(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate(adapter, access, HostMutationOp::Rem, value, source_span)
    }

    pub fn push(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.mutate(adapter, access, HostMutationOp::Push, value, source_span)
    }

    pub fn remove(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let args = self.borrowed_args();
        let target = HostTargetInstance::new(self.root, self.target(), &args);
        access.remove(adapter, target, source_span)
    }

    pub fn call_method(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        method: HostMethodId,
        args: Vec<HostValue>,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        let target_args = self.borrowed_args();
        let target = HostTargetInstance::new(self.root, self.target(), &target_args);
        access.call(adapter, target, method, &args, source_span)
    }

    fn mutate(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        op: HostMutationOp,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let args = self.borrowed_args();
        let target = HostTargetInstance::new(self.root, self.target(), &args);
        access.mutate(adapter, target, op, value, source_span)
    }

    fn borrowed_args(&self) -> Vec<HostPathArg<'_>> {
        self.args.iter().map(HostPathArg::from).collect()
    }

    fn push_arg(&mut self, arg: HostPathArgOwned) -> u8 {
        let index =
            u8::try_from(self.args.len()).expect("path proxy supports at most 256 dynamic args");
        self.args.push(arg);
        index
    }
}
