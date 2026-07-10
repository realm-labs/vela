use std::collections::{BTreeMap, btree_map::Entry};

use vela_common::{HostMethodId, HostTypeId, ShapeId};
use vela_def::{FieldId, FunctionId, GlobalId, MethodId, TypeId, VariantId};

use crate::{CompileSignature, MethodExecutableTarget, MirTypeContract};

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileTypeClass {
    ScriptRecord,
    ScriptEnum,
    Registry,
    Standard,
    Host { runtime: HostTypeId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTypeDescriptor {
    pub id: TypeId,
    pub canonical_name: String,
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
    pub writable: bool,
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
