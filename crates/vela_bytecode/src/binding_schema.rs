//! Deterministic, language-neutral input for generated Rust bindings.
//!
//! The schema is produced from the same HIR graph and verified MIR generation
//! used for bytecode linking. Runtime grants, allowlists, reflection policy,
//! budgets, and other deployment state are intentionally absent.

use std::collections::{BTreeMap, BTreeSet};

use vela_common::{CallableAsyncness, Capability, CapabilitySet, Span, stable_id};
use vela_def::{FunctionId, MethodId, TypeId};
use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, Visibility};
use vela_hir::script_methods::ScriptMethod;
use vela_hir::type_hint::{FunctionSignature, HirTypeHint};
use vela_mir::{MirAwaitOperation, MirCall, MirEffect, MirStatementKind, MirTerminatorKind};
use vela_package::{ModulePath, PackageId};

pub const RUST_BINDING_SCHEMA_VERSION: u32 = 1;

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
    pub callables: Box<[RustBindingCallable]>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustBindingMethodOwner {
    pub type_id: TypeId,
    pub public_path: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RustBindingBoundaryMode {
    Value,
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
    Path {
        segments: Box<[String]>,
        arguments: Box<[RustBindingType]>,
    },
}

impl RustBindingType {
    fn from_hint(hint: Option<&HirTypeHint>) -> Self {
        hint.map_or(Self::Any, |hint| Self::Path {
            segments: hint.path.clone().into_boxed_slice(),
            arguments: hint
                .args
                .iter()
                .map(|argument| Self::from_hint(Some(argument)))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        })
    }

    fn encode(&self, output: &mut String) {
        match self {
            Self::Any => output.push_str("any"),
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
    Value,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RustBindingErrorMode {
    VmResult,
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

    #[must_use]
    pub fn callable(&self, identity: RustBindingCallableIdentity) -> Option<&RustBindingCallable> {
        self.callables()
            .find(|callable| callable.identity == identity)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn empty() -> Self {
        Self {
            version: RUST_BINDING_SCHEMA_VERSION,
            checksum: stable_id("vela_rust_binding_schema_v1", "", ""),
            packages: Box::new([]),
        }
    }
}

pub(crate) fn build_rust_binding_schema(
    graph: &ModuleGraph,
    function_symbols: &BTreeMap<HirDeclId, String>,
    methods: &[ScriptMethod],
    bundle: &vela_mir::OwnedVerifiedMirBundle,
) -> Result<RustBindingSchema, String> {
    let effects = effective_effects(bundle);
    let method_owners = method_owners(bundle)?;
    let mut modules = BTreeMap::<(PackageId, ModulePath), Vec<RustBindingCallable>>::new();

    for (declaration, symbol) in function_symbols {
        let metadata = graph
            .declaration(*declaration)
            .ok_or_else(|| "binding-schema function has no declaration metadata".to_owned())?;
        if metadata.kind != DeclarationKind::Function || metadata.visibility != Visibility::Public {
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
            effects.get(&executable).copied().unwrap_or(MirEffect::PURE),
            docs(graph.declaration_attrs(*declaration)),
            metadata.span,
        );
        modules.entry((package, module)).or_default().push(callable);
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
            effects.get(&executable).copied().unwrap_or(MirEffect::PURE),
            None,
            method.origin().span,
        );
        modules.entry((package, module)).or_default().push(callable);
    }

    let mut packages = BTreeMap::<PackageId, Vec<RustBindingModule>>::new();
    for ((package, path), mut callables) in modules {
        callables.sort_by(|left, right| {
            left.public_path
                .cmp(&right.public_path)
                .then(left.identity.cmp(&right.identity))
        });
        packages
            .entry(package)
            .or_default()
            .push(RustBindingModule {
                path,
                callables: callables.into_boxed_slice(),
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
    let canonical = packages
        .iter()
        .flat_map(|package| {
            package.modules.iter().flat_map(move |module| {
                module.callables.iter().map(move |callable| {
                    format!(
                        "{}:{}:{}:{:016x}",
                        package.id,
                        module.path.join(),
                        callable.identity.abi_name(),
                        callable.contract_fingerprint
                    )
                })
            })
        })
        .collect::<Vec<_>>()
        .join("|");
    Ok(RustBindingSchema {
        version: RUST_BINDING_SCHEMA_VERSION,
        checksum: stable_id("vela_rust_binding_schema_v1", "", &canonical),
        packages,
    })
}

#[allow(clippy::too_many_arguments)]
fn callable(
    identity: RustBindingCallableIdentity,
    executable: FunctionId,
    public_path: String,
    rust_name: String,
    owner: Option<RustBindingMethodOwner>,
    signature: &FunctionSignature,
    effect: MirEffect,
    docs: Option<String>,
    source: Span,
) -> RustBindingCallable {
    let parameters = signature
        .params
        .iter()
        .map(|parameter| RustBindingParameter {
            identity: stable_id("callable_parameter", &public_path, &parameter.name),
            name: parameter.name.clone(),
            ty: RustBindingType::from_hint(parameter.type_hint.as_ref()),
            mode: RustBindingBoundaryMode::Value,
            default: parameter
                .default_value_span
                .map_or(RustBindingParameterDefault::Required, |source| {
                    RustBindingParameterDefault::VelaExpression { source }
                }),
            source: parameter.span,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let effects = RustBindingEffectSet::from(effect);
    let returns = RustBindingReturn {
        ty: RustBindingType::from_hint(signature.return_type.as_ref()),
        mode: RustBindingReturnMode::Value,
        error_mode: RustBindingErrorMode::VmResult,
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
        | MirCall::DynamicMethod { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vela_common::{CallableAsyncness, SourceId};

    use super::{
        RUST_BINDING_SCHEMA_VERSION, RustBindingCallableIdentity, RustBindingParameterDefault,
        RustBindingType,
    };
    use crate::compiler::compile_test_program;

    #[test]
    fn schema_exports_public_functions_and_methods_with_structural_contracts() {
        let program = compile_test_program(
            SourceId::new(501),
            r#"
#[doc("Calculate a value")]
pub async fn calculate(input: Array<i64>, scale: i64 = 2) -> Result<i64, String> {
    return scale;
}

fn hidden() { return 0; }

pub struct Counter { value: i64 }

impl Counter {
    pub fn add(self, amount: i64) -> i64 { return amount; }
    fn secret(self) -> i64 { return 0; }
}
"#,
        )
        .expect("binding schema source should compile");
        let schema = program.binding_schema();

        assert_eq!(schema.version(), RUST_BINDING_SCHEMA_VERSION);
        let paths = schema
            .callables()
            .map(|callable| callable.public_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, ["Counter::add", "calculate"]);
        assert!(schema.callables().all(|callable| matches!(
            callable.identity,
            RustBindingCallableIdentity::Function(_) | RustBindingCallableIdentity::Method { .. }
        )));

        let calculate = schema
            .callables()
            .find(|callable| callable.public_path == "calculate")
            .expect("public function binding");
        assert_eq!(calculate.asyncness, CallableAsyncness::Async);
        assert_eq!(calculate.docs.as_deref(), Some("Calculate a value"));
        assert_eq!(calculate.source.source, SourceId::new(501));
        assert_eq!(calculate.parameters.len(), 2);
        assert_eq!(
            calculate.parameters[0].ty,
            RustBindingType::Path {
                segments: Box::new(["Array".to_owned()]),
                arguments: Box::new([RustBindingType::Path {
                    segments: Box::new(["i64".to_owned()]),
                    arguments: Box::new([]),
                }]),
            }
        );
        assert!(matches!(
            calculate.parameters[1].default,
            RustBindingParameterDefault::VelaExpression { .. }
        ));
        assert_eq!(
            calculate.returns.ty,
            RustBindingType::Path {
                segments: Box::new(["Result".to_owned()]),
                arguments: Box::new([
                    RustBindingType::Path {
                        segments: Box::new(["i64".to_owned()]),
                        arguments: Box::new([]),
                    },
                    RustBindingType::Path {
                        segments: Box::new(["String".to_owned()]),
                        arguments: Box::new([]),
                    },
                ]),
            }
        );

        let method = schema
            .callables()
            .find(|callable| callable.public_path == "Counter::add")
            .expect("public method binding");
        assert!(matches!(
            method.identity,
            RustBindingCallableIdentity::Method { .. }
        ));
        assert_eq!(
            method
                .owner
                .as_ref()
                .map(|owner| owner.public_path.as_str()),
            Some("Counter")
        );
    }

    #[test]
    fn schema_fingerprint_excludes_source_movement_but_tracks_contract_changes() {
        let first = compile_test_program(
            SourceId::new(502),
            "pub fn score(value: i64 = 1) -> i64 { return value + 1; }",
        )
        .expect("first schema");
        let moved = compile_test_program(
            SourceId::new(503),
            "\n\n\npub fn score(value: i64 = 1) -> i64 { return value + 2; }",
        )
        .expect("moved schema");
        let changed = compile_test_program(
            SourceId::new(504),
            "pub fn score(value: String = \"1\") -> i64 { return 1; }",
        )
        .expect("changed schema");

        assert_eq!(
            first.binding_schema().checksum(),
            moved.binding_schema().checksum()
        );
        assert_ne!(
            first.binding_schema().checksum(),
            changed.binding_schema().checksum()
        );
        assert_ne!(
            first
                .binding_schema()
                .callables()
                .next()
                .expect("first")
                .source,
            moved
                .binding_schema()
                .callables()
                .next()
                .expect("moved")
                .source
        );
    }

    #[test]
    fn schema_uses_transitive_effects_and_is_carried_into_linked_artifact() {
        let program = compile_test_program(
            SourceId::new(505),
            r#"
state counter: i64 = 1;
fn read_counter() { return counter; }
pub fn current() { return read_counter(); }
"#,
        )
        .expect("effect schema source should compile");
        let schema = Arc::clone(program.binding_schema());
        let current = schema.callables().next().expect("current binding");
        assert!(current.effects.script_call);
        assert!(current.effects.state_read);

        let artifact = crate::Linker::new()
            .link_compiled_program(program)
            .expect("binding schema program should link");
        assert!(Arc::ptr_eq(&schema, artifact.binding_schema()));
    }
}
