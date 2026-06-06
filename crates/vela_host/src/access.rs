use vela_common::{HostMethodId, Span};

use crate::{
    adapter::ScriptStateAdapter,
    add_values, div_values,
    error::{HostError, HostErrorKind, HostResult},
    mul_values,
    path::{HostPath, HostRef},
    rem_values, sub_values,
    value::HostValue,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostObjectSnapshot {
    pub type_id: vela_common::HostTypeId,
    pub object_id: vela_common::HostObjectId,
    pub generation: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HostAccess;

impl HostAccess {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    pub fn read_path(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        path: &HostPath,
    ) -> HostResult<HostValue> {
        self.read_path_at(adapter, path, None)
    }

    pub fn read_path_at(
        &self,
        adapter: &(impl ScriptStateAdapter + ?Sized),
        path: &HostPath,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        adapter
            .read_path(path)
            .map_err(|error| error.with_source_span_if_absent(source_span))
    }

    pub fn set_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        adapter
            .write_path(&path, value)
            .map_err(|error| error.with_source_span_if_absent(source_span))
    }

    pub fn add_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.write_compound_path(adapter, path, value, source_span, CompoundWrite::Add)
    }

    pub fn sub_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.write_compound_path(adapter, path, value, source_span, CompoundWrite::Sub)
    }

    pub fn mul_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.write_compound_path(adapter, path, value, source_span, CompoundWrite::Mul)
    }

    pub fn div_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.write_compound_path(adapter, path, value, source_span, CompoundWrite::Div)
    }

    pub fn rem_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.write_compound_path(adapter, path, value, source_span, CompoundWrite::Rem)
    }

    pub fn push_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        self.write_compound_path(adapter, path, value, source_span, CompoundWrite::Push)
    }

    pub fn remove_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        source_span: Option<Span>,
    ) -> HostResult<()> {
        adapter
            .remove_path(&path)
            .map_err(|error| error.with_source_span_if_absent(source_span))
    }

    pub fn call_method(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        method: HostMethodId,
        args: Vec<HostValue>,
        source_span: Option<Span>,
    ) -> HostResult<HostValue> {
        let result = adapter
            .call_method(&path, method, &args)
            .map_err(|error| error.with_source_span_if_absent(source_span))?;
        Ok(result)
    }

    pub fn require_fresh_ref(host_ref: HostRef, snapshot: &HostObjectSnapshot) -> HostResult<()> {
        if host_ref.type_id != snapshot.type_id {
            return Err(HostError::new(HostErrorKind::TypeMismatch {
                expected: host_ref.type_id,
                actual: snapshot.type_id,
            }));
        }
        if host_ref.object_id != snapshot.object_id {
            return Err(HostError::new(HostErrorKind::ObjectMismatch {
                expected: host_ref.object_id,
                actual: snapshot.object_id,
            }));
        }
        if host_ref.generation != snapshot.generation {
            return Err(HostError::new(HostErrorKind::StaleGeneration {
                expected: host_ref.generation,
                actual: snapshot.generation,
            }));
        }
        Ok(())
    }

    fn write_compound_path(
        &mut self,
        adapter: &mut (impl ScriptStateAdapter + ?Sized),
        path: HostPath,
        value: HostValue,
        source_span: Option<Span>,
        write: CompoundWrite,
    ) -> HostResult<()> {
        let current = adapter
            .read_path(&path)
            .map_err(|error| error.with_source_span_if_absent(source_span))?;
        let next = write.compute(&current, &value).ok_or_else(|| {
            write
                .invalid_error(path.clone())
                .with_source_span(source_span)
        })?;
        adapter
            .write_path(&path, next)
            .map_err(|error| error.with_source_span_if_absent(source_span))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompoundWrite {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Push,
}

impl CompoundWrite {
    fn compute(self, current: &HostValue, value: &HostValue) -> Option<HostValue> {
        match self {
            Self::Add => add_values(current, value),
            Self::Sub => sub_values(current, value),
            Self::Mul => mul_values(current, value),
            Self::Div => div_values(current, value),
            Self::Rem => rem_values(current, value),
            Self::Push => None,
        }
    }

    fn invalid_error(self, path: HostPath) -> HostError {
        match self {
            Self::Add => HostError::new(HostErrorKind::InvalidAdd { path }),
            Self::Sub => HostError::new(HostErrorKind::InvalidSub { path }),
            Self::Mul => HostError::new(HostErrorKind::InvalidMul { path }),
            Self::Div => HostError::new(HostErrorKind::InvalidDiv { path }),
            Self::Rem => HostError::new(HostErrorKind::InvalidRem { path }),
            Self::Push => HostError::new(HostErrorKind::InvalidPush { path }),
        }
    }
}
