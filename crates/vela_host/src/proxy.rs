use vela_common::{FieldId, HostMethodId, Span};

use crate::{
    access::HostAccess, adapter::ScriptStateAdapter, error::HostResult, path::HostPath,
    value::HostValue,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathProxy {
    path: HostPath,
}

impl PathProxy {
    #[must_use]
    pub fn new(path: HostPath) -> Self {
        Self { path }
    }

    #[must_use]
    pub fn path(&self) -> &HostPath {
        &self.path
    }

    #[must_use]
    pub fn into_path(self) -> HostPath {
        self.path
    }

    #[must_use]
    pub fn field(mut self, field: FieldId) -> Self {
        self.path = self.path.field(field);
        self
    }

    #[must_use]
    pub fn index(mut self, index: u32) -> Self {
        self.path = self.path.index(index);
        self
    }

    #[must_use]
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.path = self.path.key(key);
        self
    }

    pub fn read(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &HostAccess,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        access.read_path_at(adapter, &self.path, source_span)
    }

    pub fn set(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        access.set_path(adapter, self.path.clone(), value, source_span)
    }

    pub fn add(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        access.add_path(adapter, self.path.clone(), value, source_span)
    }

    pub fn sub(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        access.sub_path(adapter, self.path.clone(), value, source_span)
    }

    pub fn mul(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        access.mul_path(adapter, self.path.clone(), value, source_span)
    }

    pub fn div(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        access.div_path(adapter, self.path.clone(), value, source_span)
    }

    pub fn rem(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        access.rem_path(adapter, self.path.clone(), value, source_span)
    }

    pub fn push(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        access.push_path(adapter, self.path.clone(), value, source_span)
    }

    pub fn remove(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        access.remove_path(adapter, self.path.clone(), source_span)
    }

    pub fn call_method(
        &self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        access: &mut HostAccess,
        method: HostMethodId,
        args: Vec<HostValue>,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        access.call_method(adapter, self.path.clone(), method, args, source_span)
    }
}
