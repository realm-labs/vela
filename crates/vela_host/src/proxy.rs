use vela_common::{FieldId, HostMethodId, Span, Symbol};

use crate::{HostPath, HostResult, HostValue, PatchTx, ScriptStateAdapter};

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
    pub fn key(mut self, key: Symbol) -> Self {
        self.path = self.path.key(key);
        self
    }

    pub fn read(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        tx: &PatchTx,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        tx.read_path_at(adapter, &self.path, source_span)
    }

    pub fn set(
        &self,
        tx: &mut PatchTx,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        tx.set_path(self.path.clone(), value, source_span)
    }

    pub fn add(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        tx: &mut PatchTx,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let base_value = tx.read_path_at(adapter, &self.path, source_span)?;
        tx.add_path(self.path.clone(), value, base_value, source_span)
    }

    pub fn sub(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        tx: &mut PatchTx,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let base_value = tx.read_path_at(adapter, &self.path, source_span)?;
        tx.sub_path(self.path.clone(), value, base_value, source_span)
    }

    pub fn push(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        tx: &mut PatchTx,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        let base_value = tx.read_path_at(adapter, &self.path, source_span)?;
        tx.push_path(self.path.clone(), value, base_value, source_span)
    }

    pub fn remove(&self, tx: &mut PatchTx, source_span: Option<Span>) -> HostResult<()> {
        tx.remove_path(self.path.clone(), source_span)
    }

    pub fn call_method(
        &self,
        tx: &mut PatchTx,
        method: HostMethodId,
        args: Vec<HostValue>,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        tx.call_method(self.path.clone(), method, args, source_span)
    }
}
