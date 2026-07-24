//! Deterministic, language-neutral input for generated Rust bindings.
//!
//! The schema is produced from the same HIR graph and verified MIR generation
//! used for bytecode linking. Runtime grants, allowlists, reflection policy,
//! budgets, and other deployment state are intentionally absent.

use std::collections::{BTreeMap, BTreeSet};

use vela_common::{CallableAsyncness, Capability, CapabilitySet, Span, stable_id};
use vela_def::{FunctionId, MethodId, TypeId};
use vela_hir::attributes::schema_id_attr;
use vela_hir::ids::HirDeclId;
use vela_hir::ids::ModuleId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, Visibility};
use vela_hir::script_methods::ScriptMethod;
use vela_hir::type_hint::{EnumVariantFieldsHint, FunctionSignature, HirTypeHint, StructFieldHint};
use vela_mir::{MirAwaitOperation, MirCall, MirEffect, MirStatementKind, MirTerminatorKind};
use vela_package::{ModulePath, PackageId};

pub const RUST_BINDING_SCHEMA_VERSION: u32 = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingSchema {
    version: u32,
    checksum: u64,
    packages: Box<[RustBindingPackage]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingPackage {
    pub id: PackageId,
    pub modules: Box<[RustBindingModule]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingModule {
    pub path: ModulePath,
    pub types: Box<[RustBindingTypeDefinition]>,
    pub callables: Box<[RustBindingCallable]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustBindingTypeDefinition {
    Record(RustBindingRecord),
    Enum(RustBindingEnum),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingRecord {
    pub type_id: TypeId,
    pub schema_fingerprint: u64,
    pub public_path: String,
    pub rust_name: String,
    pub fields: Box<[RustBindingField]>,
    pub docs: Option<String>,
    pub source: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingEnum {
    pub type_id: TypeId,
    pub schema_fingerprint: u64,
    pub public_path: String,
    pub rust_name: String,
    pub variants: Box<[RustBindingVariant]>,
    pub docs: Option<String>,
    pub source: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingField {
    pub name: String,
    pub ty: RustBindingType,
    pub source: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingVariant {
    pub name: String,
    pub fields: RustBindingVariantFields,
    pub source: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustBindingVariantFields {
    Unit,
    Tuple(Box<[RustBindingType]>),
    Record(Box<[RustBindingField]>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RustBindingCallableIdentity {
    Function(FunctionId),
    Method { owner: TypeId, method: MethodId },
}

impl RustBindingCallableIdentity {
    fn encode(self, output: &mut String) {
        match self {
            Self::Function(id) => output.push_str(&format!("f:{:032x}", id.get())),
            Self::Method { owner, method } => {
                output.push_str(&format!("m:{:032x}:{:032x}", owner.get(), method.get()))
            }
        }
    }

    const fn abi_name(self) -> &'static str {
        match self {
            Self::Function(_) => "function",
            Self::Method { .. } => "method",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingCallable {
    pub identity: RustBindingCallableIdentity,
    pub executable: FunctionId,
    pub public_path: String,
    pub rust_name: String,
    pub owner: Option<RustBindingMethodOwner>,
    pub parameters: Box<[RustBindingParameter]>,
    pub returns: RustBindingReturn,
    pub asyncness: CallableAsyncness,
    pub effects: RustBindingEffectSet,
    pub required_capabilities: CapabilitySet,
    pub contract_fingerprint: u64,
    pub docs: Option<String>,
    pub source: Span,
}

impl RustBindingCallable {
    #[doc(hidden)]
    pub fn refresh_contract_fingerprint(&mut self) {
        self.contract_fingerprint = contract_fingerprint(
            self.identity,
            &self.public_path,
            &self.parameters,
            &self.returns,
            self.asyncness,
            self.effects,
        );
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingMethodOwner {
    pub type_id: TypeId,
    pub public_path: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RustBindingBoundaryMode {
    Value,
    SharedHost,
    ExclusiveHost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingParameter {
    pub identity: u64,
    pub name: String,
    pub ty: RustBindingType,
    pub mode: RustBindingBoundaryMode,
    pub default: RustBindingParameterDefault,
    pub source: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustBindingParameterDefault {
    Required,
    VelaExpression { source: Span },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RustBindingType {
    Any,
    Host {
        semantic_type_id: TypeId,
        runtime_type_id: vela_common::HostTypeId,
        public_path: String,
    },
    Definition {
        type_id: TypeId,
        public_path: String,
    },
    Path {
        segments: Box<[String]>,
        arguments: Box<[RustBindingType]>,
    },
}

impl RustBindingType {
    fn from_hint(
        graph: &ModuleGraph,
        module: ModuleId,
        type_symbols: &BTreeMap<HirDeclId, String>,
        hint: Option<&HirTypeHint>,
    ) -> Self {
        hint.map_or(Self::Any, |hint| {
            let definition = [DeclarationKind::Struct, DeclarationKind::Enum]
                .into_iter()
                .find_map(|kind| graph.resolve_visible_declaration_path(module, &hint.path, kind));
            if let Some(definition) = definition
                && let Some(symbol) = type_symbols.get(&definition.id)
                && let Some(package) = graph.module_package(definition.module)
            {
                let explicit =
                    schema_id_attr(graph.declaration_attrs(definition.id)).map(u128::from);
                return Self::Definition {
                    type_id: vela_def::script_type_id(package.as_str(), symbol, explicit),
                    public_path: symbol.clone(),
                };
            }
            Self::Path {
                segments: hint.path.clone().into_boxed_slice(),
                arguments: hint
                    .args
                    .iter()
                    .map(|argument| Self::from_hint(graph, module, type_symbols, Some(argument)))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            }
        })
    }

    fn encode(&self, output: &mut String) {
        match self {
            Self::Any => output.push_str("any"),
            Self::Host {
                semantic_type_id,
                runtime_type_id,
                public_path,
            } => output.push_str(&format!(
                "host:{:032x}:{:016x}:{public_path}",
                semantic_type_id.get(),
                runtime_type_id.get()
            )),
            Self::Definition {
                type_id,
                public_path,
            } => output.push_str(&format!("def:{:032x}:{public_path}", type_id.get())),
            Self::Path {
                segments,
                arguments,
            } => {
                output.push_str(&segments.join("::"));
                output.push('<');
                for argument in arguments {
                    argument.encode(output);
                    output.push(',');
                }
                output.push('>');
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RustBindingReturnMode {
    OwnedValue,
    StructuredValue,
    ScopedHost {
        origin: RustBindingBorrowedReturnOrigin,
        child_access: RustBindingScopedHostAccess,
        parent_freeze: RustBindingScopedHostAccess,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RustBindingErrorMode {
    Value,
    RuntimeResult,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RustBindingBorrowedReturnOrigin {
    Receiver,
    Parameter(u16),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RustBindingScopedHostAccess {
    Shared,
    Exclusive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingReturn {
    pub ty: RustBindingType,
    pub mode: RustBindingReturnMode,
    pub error_mode: RustBindingErrorMode,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RustBindingEffectSet {
    pub may_trap: bool,
    pub may_allocate: bool,
    pub script_call: bool,
    pub dynamic_call: bool,
    pub state_read: bool,
    pub state_write: bool,
    pub host_read: bool,
    pub host_write: bool,
    pub host_call: bool,
    pub reflection_read: bool,
    pub reflection_write: bool,
    pub reflection_call: bool,
    pub emits_event: bool,
    pub reads_time: bool,
    pub uses_random: bool,
    pub reads_io: bool,
    pub writes_io: bool,
}

impl From<MirEffect> for RustBindingEffectSet {
    fn from(value: MirEffect) -> Self {
        Self {
            may_trap: value.may_trap,
            may_allocate: value.may_allocate,
            script_call: value.script_call,
            dynamic_call: value.dynamic_call,
            state_read: value.state_read,
            state_write: value.state_write,
            host_read: value.host_read,
            host_write: value.host_write,
            host_call: value.host_call,
            reflection_read: value.reflection_read,
            reflection_write: value.reflection_write,
            reflection_call: value.reflection_call,
            emits_event: value.emits_event,
            reads_time: value.reads_time,
            uses_random: value.uses_random,
            reads_io: value.reads_io,
            writes_io: value.writes_io,
        }
    }
}

impl RustBindingEffectSet {
    #[must_use]
    pub fn required_capabilities(self) -> CapabilitySet {
        let mut capabilities = CapabilitySet::new();
        for (present, capability) in [
            (self.host_read, Capability::HostRead),
            (self.host_write, Capability::HostWrite),
            (self.emits_event, Capability::EventEmit),
            (self.reads_time, Capability::Time),
            (self.uses_random, Capability::Random),
            (self.reads_io, Capability::IoRead),
            (self.writes_io, Capability::IoWrite),
            (self.reflection_read, Capability::ReflectionRead),
            (self.reflection_write, Capability::ReflectionWrite),
            (self.reflection_call, Capability::ReflectionCall),
        ] {
            if present {
                capabilities.insert(capability);
            }
        }
        if capabilities.contains(Capability::HostWrite) {
            capabilities = capabilities.without(Capability::HostRead);
        }
        capabilities
    }

    #[must_use]
    pub fn bits(self) -> u32 {
        let flags = [
            self.may_trap,
            self.may_allocate,
            self.script_call,
            self.dynamic_call,
            self.state_read,
            self.state_write,
            self.host_read,
            self.host_write,
            self.host_call,
            self.reflection_read,
            self.reflection_write,
            self.reflection_call,
            self.emits_event,
            self.reads_time,
            self.uses_random,
            self.reads_io,
            self.writes_io,
        ];
        flags.iter().enumerate().fold(0_u32, |bits, (index, set)| {
            bits | (u32::from(*set) << index)
        })
    }
}

impl RustBindingSchema {
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub const fn checksum(&self) -> u64 {
        self.checksum
    }

    #[must_use]
    pub fn packages(&self) -> &[RustBindingPackage] {
        &self.packages
    }

    pub fn callables(&self) -> impl Iterator<Item = &RustBindingCallable> {
        self.packages
            .iter()
            .flat_map(|package| &package.modules)
            .flat_map(|module| &module.callables)
    }

    #[doc(hidden)]
    pub fn callables_mut(&mut self) -> impl Iterator<Item = &mut RustBindingCallable> {
        self.packages
            .iter_mut()
            .flat_map(|package| &mut package.modules)
            .flat_map(|module| &mut module.callables)
    }

    #[doc(hidden)]
    pub fn refresh_checksum(&mut self) {
        self.checksum = schema_checksum(&self.packages);
    }

    pub fn types(&self) -> impl Iterator<Item = &RustBindingTypeDefinition> {
        self.packages
            .iter()
            .flat_map(|package| &package.modules)
            .flat_map(|module| &module.types)
    }

    #[must_use]
    pub fn type_definition(&self, type_id: TypeId) -> Option<&RustBindingTypeDefinition> {
        self.types().find(|definition| match definition {
            RustBindingTypeDefinition::Record(record) => record.type_id == type_id,
            RustBindingTypeDefinition::Enum(item) => item.type_id == type_id,
        })
    }

    #[must_use]
    pub fn callable(&self, identity: RustBindingCallableIdentity) -> Option<&RustBindingCallable> {
        self.callables()
            .find(|callable| callable.identity == identity)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn empty() -> Self {
        Self {
            version: RUST_BINDING_SCHEMA_VERSION,
            checksum: stable_id("vela_rust_binding_schema_v5", "", ""),
            packages: Box::new([]),
        }
    }
}

pub(crate) fn build_rust_binding_schema(
    graph: &ModuleGraph,
    function_symbols: &BTreeMap<HirDeclId, String>,
    type_symbols: &BTreeMap<HirDeclId, String>,
    methods: &[ScriptMethod],
    bundle: &vela_mir::OwnedVerifiedMirBundle,
) -> Result<RustBindingSchema, String> {
    let effects = effective_effects(bundle);
    let method_owners = method_owners(bundle)?;
    #[derive(Default)]
    struct ModuleBindings {
        types: Vec<RustBindingTypeDefinition>,
        callables: Vec<RustBindingCallable>,
    }
    let mut modules = BTreeMap::<(PackageId, ModulePath), ModuleBindings>::new();

    for (declaration, symbol) in type_symbols {
        let metadata = graph
            .declaration(*declaration)
            .ok_or_else(|| "binding-schema type has no declaration metadata".to_owned())?;
        if metadata.visibility != Visibility::Public {
            continue;
        }
        let package = graph
            .module_package(metadata.module)
            .cloned()
            .ok_or_else(|| "binding-schema type has no package owner".to_owned())?;
        let module = graph
            .module_path(metadata.module)
            .cloned()
            .ok_or_else(|| "binding-schema type has no module owner".to_owned())?;
        let explicit = schema_id_attr(graph.declaration_attrs(*declaration)).map(u128::from);
        let type_id = vela_def::script_type_id(package.as_str(), symbol, explicit);
        let mut definition = match metadata.kind {
            DeclarationKind::Struct => RustBindingTypeDefinition::Record(RustBindingRecord {
                type_id,
                schema_fingerprint: 0,
                public_path: symbol.clone(),
                rust_name: metadata.name.clone(),
                fields: graph
                    .struct_shape(*declaration)
                    .ok_or_else(|| "binding-schema record has no shape".to_owned())?
                    .fields
                    .iter()
                    .map(|field| binding_field(graph, metadata.module, type_symbols, field))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                docs: docs(graph.declaration_attrs(*declaration)),
                source: metadata.span,
            }),
            DeclarationKind::Enum => RustBindingTypeDefinition::Enum(RustBindingEnum {
                type_id,
                schema_fingerprint: 0,
                public_path: symbol.clone(),
                rust_name: metadata.name.clone(),
                variants: graph
                    .enum_shape(*declaration)
                    .ok_or_else(|| "binding-schema enum has no shape".to_owned())?
                    .variants
                    .iter()
                    .map(|variant| RustBindingVariant {
                        name: variant.name.clone(),
                        fields: match &variant.fields {
                            EnumVariantFieldsHint::Unit => RustBindingVariantFields::Unit,
                            EnumVariantFieldsHint::Tuple(fields) => {
                                RustBindingVariantFields::Tuple(
                                    fields
                                        .iter()
                                        .map(|field| {
                                            RustBindingType::from_hint(
                                                graph,
                                                metadata.module,
                                                type_symbols,
                                                field.type_hint.as_ref(),
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                        .into_boxed_slice(),
                                )
                            }
                            EnumVariantFieldsHint::Record(fields) => {
                                RustBindingVariantFields::Record(
                                    fields
                                        .iter()
                                        .map(|field| {
                                            binding_field(
                                                graph,
                                                metadata.module,
                                                type_symbols,
                                                field,
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                        .into_boxed_slice(),
                                )
                            }
                        },
                        source: variant.span,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                docs: docs(graph.declaration_attrs(*declaration)),
                source: metadata.span,
            }),
            _ => continue,
        };
        let fingerprint = type_definition_fingerprint(&definition);
        match &mut definition {
            RustBindingTypeDefinition::Record(record) => {
                record.schema_fingerprint = fingerprint;
            }
            RustBindingTypeDefinition::Enum(item) => {
                item.schema_fingerprint = fingerprint;
            }
        }
        modules
            .entry((package, module))
            .or_default()
            .types
            .push(definition);
    }

    for (declaration, symbol) in function_symbols {
        let metadata = graph
            .declaration(*declaration)
            .ok_or_else(|| "binding-schema function has no declaration metadata".to_owned())?;
        if metadata.kind != DeclarationKind::Function {
            continue;
        }
        if metadata.visibility != Visibility::Public {
            continue;
        }
        let package = graph
            .module_package(metadata.module)
            .cloned()
            .ok_or_else(|| "binding-schema function has no package owner".to_owned())?;
        let module = graph
            .module_path(metadata.module)
            .cloned()
            .ok_or_else(|| "binding-schema function has no module owner".to_owned())?;
        let signature = graph
            .function_signature(*declaration)
            .ok_or_else(|| "binding-schema function has no signature".to_owned())?;
        let executable = vela_def::script_function_id(package.as_str(), symbol);
        let callable = callable(
            RustBindingCallableIdentity::Function(executable),
            executable,
            symbol.clone(),
            metadata.name.clone(),
            None,
            signature,
            graph,
            metadata.module,
            type_symbols,
            runtime_signature(bundle, executable),
            effects.get(&executable).copied().unwrap_or(MirEffect::PURE),
            docs(graph.declaration_attrs(*declaration)),
            metadata.span,
        );
        modules
            .entry((package, module))
            .or_default()
            .callables
            .push(callable);
    }

    for method in methods {
        if method.visibility() != Visibility::Public {
            continue;
        }
        let package = method.owner().package().clone();
        let module = graph
            .module_path(method.module())
            .cloned()
            .ok_or_else(|| "binding-schema method has no module owner".to_owned())?;
        let method_id = method.method_id();
        let executable = vela_def::script_function_id(package.as_str(), &method.symbol_seed());
        let owner = method_owners
            .get(&executable)
            .copied()
            .ok_or_else(|| format!("binding-schema method {method_id:?} has no semantic owner"))?;
        let public_path = format!("{}::{}", method.owner().target_type(), method.name());
        let callable = callable(
            RustBindingCallableIdentity::Method {
                owner,
                method: method_id,
            },
            executable,
            public_path,
            method.name().to_owned(),
            Some(RustBindingMethodOwner {
                type_id: owner,
                public_path: method.owner().target_type().to_owned(),
            }),
            method.signature(),
            graph,
            method.module(),
            type_symbols,
            runtime_signature(bundle, executable),
            effects.get(&executable).copied().unwrap_or(MirEffect::PURE),
            None,
            method.origin().span,
        );
        modules
            .entry((package, module))
            .or_default()
            .callables
            .push(callable);
    }

    let mut packages = BTreeMap::<PackageId, Vec<RustBindingModule>>::new();
    for ((package, path), mut bindings) in modules {
        bindings.callables.sort_by(|left, right| {
            left.public_path
                .cmp(&right.public_path)
                .then(left.identity.cmp(&right.identity))
        });
        bindings
            .types
            .sort_by(|left, right| type_definition_path(left).cmp(type_definition_path(right)));
        packages
            .entry(package)
            .or_default()
            .push(RustBindingModule {
                path,
                types: bindings.types.into_boxed_slice(),
                callables: bindings.callables.into_boxed_slice(),
            });
    }
    let packages = packages
        .into_iter()
        .map(|(id, mut modules)| {
            modules.sort_by(|left, right| left.path.cmp(&right.path));
            RustBindingPackage {
                id,
                modules: modules.into_boxed_slice(),
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let checksum = schema_checksum(&packages);
    Ok(RustBindingSchema {
        version: RUST_BINDING_SCHEMA_VERSION,
        checksum,
        packages,
    })
}

fn schema_checksum(packages: &[RustBindingPackage]) -> u64 {
    let mut canonical_parts = Vec::new();
    for package in packages {
        for module in &package.modules {
            let prefix = format!("{}:{}", package.id, module.path.join());
            for definition in &module.types {
                canonical_parts.push(match definition {
                    RustBindingTypeDefinition::Record(record) => format!(
                        "{prefix}:t:{:032x}:{:016x}",
                        record.type_id.get(),
                        record.schema_fingerprint
                    ),
                    RustBindingTypeDefinition::Enum(item) => format!(
                        "{prefix}:t:{:032x}:{:016x}",
                        item.type_id.get(),
                        item.schema_fingerprint
                    ),
                });
            }
            for callable in &module.callables {
                canonical_parts.push(format!(
                    "{prefix}:{}:{:016x}",
                    callable.identity.abi_name(),
                    callable.contract_fingerprint
                ));
            }
        }
    }
    let canonical = canonical_parts.join("|");
    stable_id("vela_rust_binding_schema_v5", "", &canonical)
}

fn binding_field(
    graph: &ModuleGraph,
    module: ModuleId,
    type_symbols: &BTreeMap<HirDeclId, String>,
    field: &StructFieldHint,
) -> RustBindingField {
    RustBindingField {
        name: field.name.clone(),
        ty: RustBindingType::from_hint(graph, module, type_symbols, field.type_hint.as_ref()),
        source: field.span,
    }
}

fn type_definition_path(definition: &RustBindingTypeDefinition) -> &str {
    match definition {
        RustBindingTypeDefinition::Record(record) => &record.public_path,
        RustBindingTypeDefinition::Enum(item) => &item.public_path,
    }
}

fn type_definition_fingerprint(definition: &RustBindingTypeDefinition) -> u64 {
    let mut canonical = String::new();
    match definition {
        RustBindingTypeDefinition::Record(record) => {
            canonical.push_str("record:");
            canonical.push_str(&record.public_path);
            for field in &record.fields {
                canonical.push_str("|f:");
                canonical.push_str(&field.name);
                canonical.push(':');
                field.ty.encode(&mut canonical);
            }
        }
        RustBindingTypeDefinition::Enum(item) => {
            canonical.push_str("enum:");
            canonical.push_str(&item.public_path);
            for variant in &item.variants {
                canonical.push_str("|v:");
                canonical.push_str(&variant.name);
                match &variant.fields {
                    RustBindingVariantFields::Unit => canonical.push_str(":unit"),
                    RustBindingVariantFields::Tuple(fields) => {
                        canonical.push_str(":tuple");
                        for field in fields {
                            canonical.push(':');
                            field.encode(&mut canonical);
                        }
                    }
                    RustBindingVariantFields::Record(fields) => {
                        canonical.push_str(":record");
                        for field in fields {
                            canonical.push_str(":f:");
                            canonical.push_str(&field.name);
                            canonical.push(':');
                            field.ty.encode(&mut canonical);
                        }
                    }
                }
            }
        }
    }
    stable_id("vela_rust_binding_type_v1", "", &canonical)
}

#[allow(clippy::too_many_arguments)]
fn callable(
    identity: RustBindingCallableIdentity,
    executable: FunctionId,
    public_path: String,
    rust_name: String,
    owner: Option<RustBindingMethodOwner>,
    signature: &FunctionSignature,
    graph: &ModuleGraph,
    module: ModuleId,
    type_symbols: &BTreeMap<HirDeclId, String>,
    runtime_signature: Option<&vela_mir::CompileSignature>,
    effect: MirEffect,
    docs: Option<String>,
    source: Span,
) -> RustBindingCallable {
    let runtime_offset = usize::from(
        owner.is_some()
            && signature
                .params
                .first()
                .is_some_and(|parameter| parameter.name == "self")
            && runtime_signature.is_some_and(|runtime| {
                runtime.parameters.len().saturating_add(1) == signature.params.len()
            }),
    );
    let mut parameters = signature
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let runtime_contract = index.checked_sub(runtime_offset).and_then(|index| {
                runtime_signature
                    .and_then(|runtime| runtime.parameters.get(index))
                    .and_then(|parameter| parameter.contract.as_ref())
            });
            let (ty, mode) = binding_parameter_type(
                graph,
                module,
                type_symbols,
                parameter.type_hint.as_ref(),
                runtime_contract,
                effect,
            );
            RustBindingParameter {
                identity: stable_id("callable_parameter", &public_path, &parameter.name),
                name: parameter.name.clone(),
                ty,
                mode,
                default: parameter
                    .default_value_span
                    .map_or(RustBindingParameterDefault::Required, |source| {
                        RustBindingParameterDefault::VelaExpression { source }
                    }),
                source: parameter.span,
            }
        })
        .collect::<Vec<_>>();
    if let Some(owner) = owner.as_ref()
        && let Some(receiver) = parameters.first_mut()
        && receiver.name == "self"
    {
        receiver.ty = RustBindingType::Definition {
            type_id: owner.type_id,
            public_path: owner.public_path.clone(),
        };
    }
    let parameters = parameters.into_boxed_slice();
    let effects = RustBindingEffectSet::from(effect);
    let returns = RustBindingReturn {
        ty: RustBindingType::from_hint(graph, module, type_symbols, signature.return_type.as_ref()),
        mode: RustBindingReturnMode::OwnedValue,
        error_mode: RustBindingErrorMode::RuntimeResult,
    };
    let contract_fingerprint = contract_fingerprint(
        identity,
        &public_path,
        &parameters,
        &returns,
        signature.asyncness,
        effects,
    );
    RustBindingCallable {
        identity,
        executable,
        public_path,
        rust_name,
        owner,
        parameters,
        returns,
        asyncness: signature.asyncness,
        effects,
        required_capabilities: effects.required_capabilities(),
        contract_fingerprint,
        docs,
        source,
    }
}

fn binding_parameter_type(
    graph: &ModuleGraph,
    module: ModuleId,
    type_symbols: &BTreeMap<HirDeclId, String>,
    hint: Option<&HirTypeHint>,
    contract: Option<&vela_mir::MirTypeContract>,
    effect: MirEffect,
) -> (RustBindingType, RustBindingBoundaryMode) {
    if let Some(vela_mir::MirTypeContract::Host(target)) = contract {
        let public_path = hint
            .map(|hint| hint.path.join("::"))
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| format!("HostType_{:016x}", target.runtime.get()));
        let mode = if effect.host_write {
            RustBindingBoundaryMode::ExclusiveHost
        } else {
            RustBindingBoundaryMode::SharedHost
        };
        return (
            RustBindingType::Host {
                semantic_type_id: target.semantic,
                runtime_type_id: target.runtime,
                public_path,
            },
            mode,
        );
    }
    (
        RustBindingType::from_hint(graph, module, type_symbols, hint),
        RustBindingBoundaryMode::Value,
    )
}

fn runtime_signature(
    bundle: &vela_mir::OwnedVerifiedMirBundle,
    executable: FunctionId,
) -> Option<&vela_mir::CompileSignature> {
    bundle
        .root(executable)
        .and_then(|root| root.program().targets().function(executable))
        .map(|descriptor| &descriptor.signature)
}

fn contract_fingerprint(
    identity: RustBindingCallableIdentity,
    public_path: &str,
    parameters: &[RustBindingParameter],
    returns: &RustBindingReturn,
    asyncness: CallableAsyncness,
    effects: RustBindingEffectSet,
) -> u64 {
    let mut canonical = format!(
        "{}|{}|{:?}|{:08x}|",
        identity.abi_name(),
        public_path,
        asyncness,
        effects.bits()
    );
    identity.encode(&mut canonical);
    for parameter in parameters {
        canonical.push_str(&format!(
            "|p:{:016x}:{}:{:?}:{}:",
            parameter.identity,
            parameter.name,
            parameter.mode,
            match parameter.default {
                RustBindingParameterDefault::Required => "required",
                RustBindingParameterDefault::VelaExpression { .. } => "vela_expression",
            }
        ));
        parameter.ty.encode(&mut canonical);
    }
    canonical.push_str("|r:");
    canonical.push_str(&format!("{:?}:{:?}:", returns.mode, returns.error_mode));
    returns.ty.encode(&mut canonical);
    stable_id("vela_rust_binding_callable_v1", "", &canonical)
}

fn docs(attributes: &[vela_hir::attributes::HirAttribute]) -> Option<String> {
    let lines = attributes
        .iter()
        .filter(|attribute| attribute.name == "doc")
        .map(vela_hir::attributes::HirAttribute::string_value)
        .collect::<Vec<_>>();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn method_owners(
    bundle: &vela_mir::OwnedVerifiedMirBundle,
) -> Result<BTreeMap<FunctionId, TypeId>, String> {
    let mut owners = BTreeMap::new();
    for (_, root) in bundle.roots() {
        for (owner, method, descriptor) in root.program().targets().methods() {
            let vela_mir::CompileMethodClass::Script { executable, .. } = descriptor.class else {
                continue;
            };
            debug_assert_eq!(method, executable.method);
            debug_assert_eq!(owner, executable.owner);
            match owners.insert(executable.function, owner) {
                Some(existing) if existing != owner => {
                    return Err(format!(
                        "binding-schema method executable {:?} has conflicting owners",
                        executable.function
                    ));
                }
                Some(_) | None => {}
            }
        }
    }
    Ok(owners)
}

fn effective_effects(bundle: &vela_mir::OwnedVerifiedMirBundle) -> BTreeMap<FunctionId, MirEffect> {
    let mut effects = BTreeMap::new();
    let mut callees = BTreeMap::<FunctionId, BTreeSet<FunctionId>>::new();
    for (root_id, root) in bundle.roots() {
        let mut effect = MirEffect::PURE;
        let mut called = BTreeSet::new();
        for (_, function) in root.program().functions() {
            for (_, statement) in function.statements() {
                effect = effect.union(statement.effect);
                if let MirStatementKind::Call(call) = &statement.kind
                    && let Some(callee) = script_callee(call)
                {
                    called.insert(callee);
                }
            }
            for (_, block) in function.blocks() {
                let Some(terminator) = block.terminator() else {
                    continue;
                };
                effect = effect.union(terminator.effect);
                if let MirTerminatorKind::AwaitCall { operation, .. } = &terminator.kind
                    && let MirAwaitOperation::Call(call) = operation.as_ref()
                    && let Some(callee) = script_callee(call)
                {
                    called.insert(callee);
                }
            }
        }
        effects.insert(root_id, effect);
        callees.insert(root_id, called);
    }
    loop {
        let mut changed = false;
        for (function, called) in &callees {
            let mut effect = effects.get(function).copied().unwrap_or(MirEffect::PURE);
            for callee in called {
                effect = effect.union(effects.get(callee).copied().unwrap_or(MirEffect::PURE));
            }
            if effects.get(function) != Some(&effect) {
                effects.insert(*function, effect);
                changed = true;
            }
        }
        if !changed {
            return effects;
        }
    }
}

const fn script_callee(call: &MirCall) -> Option<FunctionId> {
    match call {
        MirCall::ScriptFunction { function, .. } => Some(*function),
        MirCall::ScriptMethod { target, .. } => Some(target.function),
        MirCall::CallableValue { .. }
        | MirCall::DynamicCallable { .. }
        | MirCall::NativeFunction { .. }
        | MirCall::StdlibFunction { .. }
        | MirCall::ValueMethod { .. }
        | MirCall::Service { .. }
        | MirCall::DynamicMethod { .. } => None,
    }
}

#[cfg(test)]
mod tests;
