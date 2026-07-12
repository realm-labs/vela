use std::collections::{BTreeMap, btree_map::Entry};

use vela_common::{HostMethodId, HostTypeId, ShapeId};
use vela_def::{FieldId, FunctionId, GlobalId, MethodId, TypeId, VariantId};

use crate::{CompileSignature, MethodExecutableTarget, MirTypeContract};

/// Function visibility copied into one immutable compile-target generation.
///
/// Capability requirements remain part of the function effect metadata. The
/// current function access contract has no named permission list, unlike
/// fields and methods.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileFunctionAccess {
    pub public: bool,
    pub reflect_visible: bool,
    pub reflect_callable: bool,
}

impl CompileFunctionAccess {
    #[must_use]
    pub const fn new(public: bool, reflect_visible: bool, reflect_callable: bool) -> Self {
        Self {
            public,
            reflect_visible,
            reflect_callable,
        }
    }

    /// Access assigned to a source script function.
    #[must_use]
    pub const fn script(public: bool) -> Self {
        Self::new(public, true, false)
    }
}

/// Method visibility and reflection policy copied from semantic metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileMethodAccess {
    pub public: bool,
    pub reflect_callable: bool,
    required_permissions: Vec<String>,
}

impl CompileMethodAccess {
    #[must_use]
    pub fn new(public: bool, reflect_callable: bool, required_permissions: Vec<String>) -> Self {
        Self {
            public,
            reflect_callable,
            required_permissions: canonical_permissions(required_permissions),
        }
    }

    /// Access assigned to a source script method.
    #[must_use]
    pub fn script() -> Self {
        Self::new(true, true, Vec::new())
    }

    #[must_use]
    pub fn required_permissions(&self) -> &[String] {
        &self.required_permissions
    }
}

/// Field read/write and reflection policy copied from semantic metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileFieldAccess {
    pub readable: bool,
    pub writable: bool,
    pub reflect_readable: bool,
    pub reflect_writable: bool,
    required_permissions: Vec<String>,
}

impl CompileFieldAccess {
    #[must_use]
    pub fn new(
        readable: bool,
        writable: bool,
        reflect_readable: bool,
        reflect_writable: bool,
        required_permissions: Vec<String>,
    ) -> Self {
        Self {
            readable,
            writable,
            reflect_readable,
            reflect_writable,
            required_permissions: canonical_permissions(required_permissions),
        }
    }

    /// Access assigned to a source script record or enum field.
    #[must_use]
    pub fn script() -> Self {
        Self::new(true, true, true, true, Vec::new())
    }

    #[must_use]
    pub fn required_permissions(&self) -> &[String] {
        &self.required_permissions
    }
}

fn canonical_permissions(mut permissions: Vec<String>) -> Vec<String> {
    permissions.sort();
    permissions.dedup();
    permissions
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileFunctionClass {
    Script,
    Native,
    Stdlib,
    Registry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileFunctionDescriptor {
    pub id: FunctionId,
    pub class: CompileFunctionClass,
    pub canonical_symbol: String,
    pub debug_name: String,
    pub signature: CompileSignature,
    pub access: CompileFunctionAccess,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileMethodClass {
    Script {
        executable: MethodExecutableTarget,
        owner_name: String,
        code_symbol: String,
    },
    Host {
        runtime: HostMethodId,
    },
    Value,
    Registry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileMethodDescriptor {
    pub id: MethodId,
    pub owner: TypeId,
    pub member_name: String,
    pub debug_name: String,
    pub class: CompileMethodClass,
    /// User parameters only. A script method receiver is described separately.
    pub signature: CompileSignature,
    pub access: CompileMethodAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileTypeClass {
    ScriptRecord,
    ScriptEnum,
    /// A stable external dispatch owner known by source identity only.
    ///
    /// Unlike `Registry`, this type has no authoritative registry definition,
    /// and unlike `Host`, it has no generation-local runtime type handle.
    OpaqueExternal,
    Registry,
    Standard,
    Host {
        runtime: HostTypeId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTypeDescriptor {
    pub id: TypeId,
    /// Stable semantic lookup identity, including its definition package.
    pub canonical_name: String,
    /// Exact name carried by runtime record/enum values and diagnostics.
    ///
    /// This is producer-owned runtime data, not a lookup alias. It may be
    /// shared by descriptors whose canonical identities differ.
    pub runtime_name: String,
    pub class: CompileTypeClass,
    pub shape: Option<ShapeId>,
    pub fields: Vec<FieldId>,
    pub variants: Vec<VariantId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileVariantDescriptor {
    pub id: VariantId,
    pub owner: TypeId,
    pub name: String,
    pub fields: Vec<FieldId>,
    pub declaration_order: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileFieldDescriptor {
    pub id: FieldId,
    pub owner: TypeId,
    pub variant: Option<VariantId>,
    pub name: String,
    pub contract: Option<MirTypeContract>,
    pub declaration_order: u32,
    pub access: CompileFieldAccess,
    pub host_runtime: Option<FieldId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileGlobalDescriptor {
    pub id: GlobalId,
    pub name: String,
    pub contract: MirTypeContract,
}

/// All canonical/debug symbols required by a backend after semantic lowering.
///
/// The table is owned by a MIR generation. Backends never recover these facts
/// from HIR, analysis, names, or a live registry.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirTargetTable {
    functions: BTreeMap<FunctionId, CompileFunctionDescriptor>,
    methods: BTreeMap<(TypeId, MethodId), CompileMethodDescriptor>,
    types: BTreeMap<TypeId, CompileTypeDescriptor>,
    variants: BTreeMap<VariantId, CompileVariantDescriptor>,
    fields: BTreeMap<FieldId, CompileFieldDescriptor>,
    globals: BTreeMap<GlobalId, CompileGlobalDescriptor>,
}

impl MirTargetTable {
    #[must_use]
    pub fn function(&self, id: FunctionId) -> Option<&CompileFunctionDescriptor> {
        self.functions.get(&id)
    }

    #[must_use]
    pub fn method(&self, owner: TypeId, id: MethodId) -> Option<&CompileMethodDescriptor> {
        self.methods.get(&(owner, id))
    }

    #[must_use]
    pub fn type_descriptor(&self, id: TypeId) -> Option<&CompileTypeDescriptor> {
        self.types.get(&id)
    }

    #[must_use]
    pub fn variant(&self, id: VariantId) -> Option<&CompileVariantDescriptor> {
        self.variants.get(&id)
    }

    #[must_use]
    pub fn field(&self, id: FieldId) -> Option<&CompileFieldDescriptor> {
        self.fields.get(&id)
    }

    #[must_use]
    pub fn global(&self, id: GlobalId) -> Option<&CompileGlobalDescriptor> {
        self.globals.get(&id)
    }

    pub fn functions(&self) -> impl Iterator<Item = (FunctionId, &CompileFunctionDescriptor)> {
        self.functions.iter().map(|(id, value)| (*id, value))
    }

    pub fn methods(&self) -> impl Iterator<Item = (TypeId, MethodId, &CompileMethodDescriptor)> {
        self.methods
            .iter()
            .map(|((owner, id), value)| (*owner, *id, value))
    }

    pub fn types(&self) -> impl Iterator<Item = (TypeId, &CompileTypeDescriptor)> {
        self.types.iter().map(|(id, value)| (*id, value))
    }

    pub fn variants(&self) -> impl Iterator<Item = (VariantId, &CompileVariantDescriptor)> {
        self.variants.iter().map(|(id, value)| (*id, value))
    }

    pub fn fields(&self) -> impl Iterator<Item = (FieldId, &CompileFieldDescriptor)> {
        self.fields.iter().map(|(id, value)| (*id, value))
    }

    pub fn globals(&self) -> impl Iterator<Item = (GlobalId, &CompileGlobalDescriptor)> {
        self.globals.iter().map(|(id, value)| (*id, value))
    }

    pub(crate) fn insert_function(&mut self, value: CompileFunctionDescriptor) -> bool {
        insert_unique(&mut self.functions, value.id, value)
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn insert_test_function(&mut self, value: CompileFunctionDescriptor) -> bool {
        self.insert_function(value)
    }

    pub(crate) fn insert_method(&mut self, value: CompileMethodDescriptor) -> bool {
        insert_unique(&mut self.methods, (value.owner, value.id), value)
    }

    pub(crate) fn insert_type(&mut self, value: CompileTypeDescriptor) -> bool {
        insert_unique(&mut self.types, value.id, value)
    }

    pub(crate) fn insert_variant(&mut self, value: CompileVariantDescriptor) -> bool {
        insert_unique(&mut self.variants, value.id, value)
    }

    pub(crate) fn insert_field(&mut self, value: CompileFieldDescriptor) -> bool {
        insert_unique(&mut self.fields, value.id, value)
    }

    pub(crate) fn insert_global(&mut self, value: CompileGlobalDescriptor) -> bool {
        insert_unique(&mut self.globals, value.id, value)
    }
}

fn insert_unique<K: Ord, V>(values: &mut BTreeMap<K, V>, key: K, value: V) -> bool {
    match values.entry(key) {
        Entry::Vacant(entry) => {
            entry.insert(value);
            true
        }
        Entry::Occupied(_) => false,
    }
}
