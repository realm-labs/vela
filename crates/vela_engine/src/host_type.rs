use vela_common::ReceiverCapability;
use vela_host::path::{HostPath, PathSegment};
use vela_host::resolved::{HostAccessOp, HostAccessSpec};
use vela_host::target::{HostTargetInstance, HostTargetPlan};
use vela_host::value::HostValue;
use vela_reflect::registry::TypeDesc;
use vela_vm::HostExecution;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::method::{AsyncNativeMethodEntry, NativeMethodDesc, NativeMethodEntry};
use crate::typed::TypedNativeMethodFunction;

#[derive(Clone)]
pub struct HostTypeSpec {
    type_desc: TypeDesc,
    method_metadata: Vec<NativeMethodDesc>,
    native_methods: Vec<NativeMethodEntry>,
    async_native_methods: Vec<AsyncNativeMethodEntry>,
}

impl HostTypeSpec {
    #[must_use]
    pub fn new(type_desc: TypeDesc) -> Self {
        Self {
            type_desc,
            method_metadata: Vec::new(),
            native_methods: Vec::new(),
            async_native_methods: Vec::new(),
        }
    }

    #[must_use]
    pub fn type_desc(&self) -> &TypeDesc {
        &self.type_desc
    }

    #[must_use]
    pub fn method_desc(mut self, desc: NativeMethodDesc) -> Self {
        self.method_metadata.push(desc);
        self
    }

    #[must_use]
    pub fn native_method_fn(
        mut self,
        desc: NativeMethodDesc,
        function: impl for<'host> Fn(
            &HostPath,
            &[OwnedValue],
            &mut HostExecution<'host>,
        ) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.native_methods
            .push(NativeMethodEntry::new(desc, function));
        self
    }

    /// Registers a synchronous method implemented by the call-scoped erased
    /// Host vtable.
    #[must_use]
    pub fn erased_method(mut self, desc: NativeMethodDesc) -> Self {
        let method = desc.id;
        self.native_methods
            .push(NativeMethodEntry::new(desc, move |receiver, args, host| {
                let plan = host_target_plan(receiver);
                let access = host
                    .adapter
                    .resolve_host_access(HostAccessSpec::new(HostAccessOp::Call(method), &plan))?;
                let args = args
                    .iter()
                    .map(owned_to_host_value)
                    .collect::<VmResult<Vec<_>>>()?;
                let target = HostTargetInstance::new(receiver.root, &plan, &[]);
                host.adapter
                    .call_host(access, target, method, &args)
                    .map(host_to_owned_value)
                    .map_err(Into::into)
            }));
        self
    }

    /// Registers an async method implemented by the call-scoped erased Host
    /// vtable. Its lease remains live until the Rust Future completes or drops.
    #[must_use]
    pub fn erased_async_method(mut self, desc: NativeMethodDesc) -> Self {
        let method = desc.id;
        let lease_kind = match desc.receiver {
            ReceiverCapability::Shared => vela_host::lease::HostLeaseKind::Shared,
            ReceiverCapability::Exclusive
            | ReceiverCapability::Owned
            | ReceiverCapability::Construct => vela_host::lease::HostLeaseKind::Exclusive,
        };
        self.async_native_methods
            .push(AsyncNativeMethodEntry::new_direct(
                desc,
                lease_kind,
                move |_root, mut lease, args| {
                    Box::pin(async move {
                        let args = args
                            .iter()
                            .map(owned_to_host_value)
                            .collect::<VmResult<Vec<_>>>()?;
                        let call = match lease_kind {
                            vela_host::lease::HostLeaseKind::Shared => {
                                lease.object().call_async_host_shared(method, args)
                            }
                            vela_host::lease::HostLeaseKind::Exclusive => lease
                                .object_send_mut()
                                .ok_or_else(|| {
                                    VmError::new(VmErrorKind::TypeMismatch {
                                        operation: "exclusive erased Host method lease",
                                    })
                                })?
                                .call_async_host_exclusive(method, args),
                        };
                        let result = call.await?;
                        Ok(host_to_owned_value(result))
                    })
                },
            ));
        self
    }

    #[must_use]
    pub fn typed_native_method_fn<Args, F>(self, desc: NativeMethodDesc, function: F) -> Self
    where
        F: TypedNativeMethodFunction<Args>,
    {
        self.native_method_fn(desc, move |receiver, args, host| {
            function.call_method(receiver, args, host)
        })
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        TypeDesc,
        Vec<NativeMethodDesc>,
        Vec<NativeMethodEntry>,
        Vec<AsyncNativeMethodEntry>,
    ) {
        (
            self.type_desc,
            self.method_metadata,
            self.native_methods,
            self.async_native_methods,
        )
    }
}

impl From<TypeDesc> for HostTypeSpec {
    fn from(type_desc: TypeDesc) -> Self {
        Self::new(type_desc)
    }
}

fn host_target_plan(receiver: &HostPath) -> HostTargetPlan {
    let mut plan =
        HostTargetPlan::with_part_capacity(receiver.root.type_id, receiver.segments.len());
    for segment in &receiver.segments {
        let part = match segment {
            PathSegment::Field(field) => vela_host::target::HostPathPart::Field(*field),
            PathSegment::VariantField(field) => {
                vela_host::target::HostPathPart::VariantField(*field)
            }
            PathSegment::Index(index) => vela_host::target::HostPathPart::ConstIndex(*index),
            PathSegment::Key(key) => vela_host::target::HostPathPart::ConstKey(key.clone()),
        };
        plan.parts.push(part);
    }
    plan
}

fn owned_to_host_value(value: &OwnedValue) -> VmResult<HostValue> {
    match value {
        OwnedValue::Unit => Ok(HostValue::Unit),
        OwnedValue::Bool(value) => Ok(HostValue::Bool(*value)),
        OwnedValue::Char(value) => Ok(HostValue::Char(*value)),
        OwnedValue::Scalar(value) => Ok(HostValue::Scalar(*value)),
        OwnedValue::String(value) => Ok(HostValue::String(value.clone())),
        OwnedValue::Bytes(value) => Ok(HostValue::Bytes(value.clone())),
        OwnedValue::HostRef(value) => Ok(HostValue::HostRef(*value)),
        _ => Err(VmError::new(VmErrorKind::TypeMismatch {
            operation: "erased Host method boundary value",
        })),
    }
}

fn host_to_owned_value(value: HostValue) -> OwnedValue {
    match value {
        HostValue::Unit => OwnedValue::Unit,
        HostValue::Bool(value) => OwnedValue::Bool(value),
        HostValue::Char(value) => OwnedValue::Char(value),
        HostValue::Scalar(value) => OwnedValue::Scalar(value),
        HostValue::String(value) => OwnedValue::String(value),
        HostValue::Bytes(value) => OwnedValue::Bytes(value),
        HostValue::HostRef(value) => OwnedValue::HostRef(value),
    }
}
