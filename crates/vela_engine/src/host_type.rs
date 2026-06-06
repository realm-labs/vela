use vela_host::path::HostPath;
use vela_reflect::registry::TypeDesc;
use vela_vm::HostExecution;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::method::{NativeMethodDesc, NativeMethodEntry};
use crate::typed::TypedNativeMethodFunction;

#[derive(Clone)]
pub struct HostTypeSpec {
    type_desc: TypeDesc,
    method_metadata: Vec<NativeMethodDesc>,
    native_methods: Vec<NativeMethodEntry>,
}

impl HostTypeSpec {
    #[must_use]
    pub fn new(type_desc: TypeDesc) -> Self {
        Self {
            type_desc,
            method_metadata: Vec::new(),
            native_methods: Vec::new(),
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

    #[must_use]
    pub fn typed_native_method_fn<Args, F>(self, desc: NativeMethodDesc, function: F) -> Self
    where
        F: TypedNativeMethodFunction<Args>,
    {
        self.native_method_fn(desc, move |receiver, args, host| {
            function.call_method(receiver, args, host)
        })
    }

    pub(crate) fn into_parts(self) -> (TypeDesc, Vec<NativeMethodDesc>, Vec<NativeMethodEntry>) {
        (self.type_desc, self.method_metadata, self.native_methods)
    }
}

impl From<TypeDesc> for HostTypeSpec {
    fn from(type_desc: TypeDesc) -> Self {
        Self::new(type_desc)
    }
}
