use std::collections::BTreeMap;

use vela_common::{FunctionId, Span};
use vela_hir::{DeclarationKind, FunctionSignature, ModuleGraph};
use vela_host::HostValue;
use vela_syntax::Visibility;

use crate::{
    AttrMap, FunctionAccess, FunctionEffectSet, ReflectError, ReflectErrorKind, ReflectPolicy,
    ReflectResult, ReflectValue, TypeRegistry,
    candidates::{candidate_names, ranked_candidates},
    metadata::{attrs_value, docs_value, span_value},
    script_attrs::ReflectedScriptAttrs,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclOrigin {
    Host,
    Script,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionParamDesc {
    pub name: String,
    pub type_hint: Option<String>,
    pub has_default: bool,
}

impl FunctionParamDesc {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_hint: None,
            has_default: false,
        }
    }

    #[must_use]
    pub fn type_hint(mut self, type_hint: impl Into<String>) -> Self {
        self.type_hint = Some(type_hint.into());
        self
    }

    #[must_use]
    pub fn defaulted(mut self, has_default: bool) -> Self {
        self.has_default = has_default;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDesc {
    pub id: FunctionId,
    pub name: String,
    pub module: Option<String>,
    pub params: Vec<FunctionParamDesc>,
    pub return_type: Option<String>,
    pub public: bool,
    pub effects: FunctionEffectSet,
    pub access: FunctionAccess,
    pub origin: DeclOrigin,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
}

impl FunctionDesc {
    #[must_use]
    pub fn new(id: FunctionId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            module: None,
            params: Vec::new(),
            return_type: None,
            public: true,
            effects: FunctionEffectSet::default(),
            access: FunctionAccess::default(),
            origin: DeclOrigin::Host,
            docs: None,
            attrs: AttrMap::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    #[must_use]
    pub fn param(mut self, param: FunctionParamDesc) -> Self {
        self.params.push(param);
        self
    }

    #[must_use]
    pub fn return_type(mut self, return_type: impl Into<String>) -> Self {
        self.return_type = Some(return_type.into());
        self
    }

    #[must_use]
    pub fn public(mut self, public: bool) -> Self {
        self.public = public;
        self.access.public = public;
        self
    }

    #[must_use]
    pub fn effects(mut self, effects: FunctionEffectSet) -> Self {
        self.effects = effects;
        self
    }

    #[must_use]
    pub fn access(mut self, access: FunctionAccess) -> Self {
        self.public = access.public;
        self.access = access;
        self
    }

    #[must_use]
    pub fn origin(mut self, origin: DeclOrigin) -> Self {
        self.origin = origin;
        self
    }

    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleExportKind {
    Function,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleExportDesc {
    pub name: String,
    pub kind: ModuleExportKind,
    pub function: Option<FunctionId>,
}

impl ModuleExportDesc {
    #[must_use]
    pub fn function(name: impl Into<String>, function: FunctionId) -> Self {
        Self {
            name: name.into(),
            kind: ModuleExportKind::Function,
            function: Some(function),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDesc {
    pub name: String,
    pub exports: Vec<ModuleExportDesc>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
}

impl ModuleDesc {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            exports: Vec::new(),
            attrs: AttrMap::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }

    pub(crate) fn export_function(&mut self, name: impl Into<String>, function: FunctionId) {
        let name = name.into();
        if self
            .exports
            .iter()
            .any(|export| export.kind == ModuleExportKind::Function && export.name == name)
        {
            return;
        }
        self.exports
            .push(ModuleExportDesc::function(name, function));
    }
}

impl TypeRegistry {
    pub fn register_script_modules(&mut self, graph: &ModuleGraph) {
        for declaration in graph.declarations() {
            let Some(module_name) = graph
                .module_path(declaration.module)
                .map(|path| path.join())
            else {
                continue;
            };
            if self.module_by_name(&module_name).is_none() {
                self.register_module(ModuleDesc::new(module_name).source_span(declaration.span));
            }
        }

        for declaration in graph.declarations() {
            if declaration.kind != DeclarationKind::Function {
                continue;
            }
            let Some(module_name) = graph
                .module_path(declaration.module)
                .map(|path| path.join())
            else {
                continue;
            };
            let qualified_name = qualified_function_name(&module_name, &declaration.name);
            let signature = graph.function_signature(declaration.id);
            let mut desc = FunctionDesc::new(
                stable_function_id(&module_name, &declaration.name),
                qualified_name,
            )
            .module(module_name)
            .public(declaration.visibility == Visibility::Public)
            .origin(DeclOrigin::Script)
            .source_span(declaration.span);
            if let Some(signature) = signature {
                desc = apply_signature(desc, signature);
            }
            desc = apply_function_attrs(desc, graph.declaration_attrs(declaration.id));
            self.register_function(desc);
        }
    }
}

pub fn module(registry: &TypeRegistry, name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.module_by_name(name).ok_or_else(|| {
        let related = module_candidates(registry, name);
        ReflectError::new(ReflectErrorKind::UnknownModule {
            module: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(module_record(desc))
}

pub fn has_module(registry: &TypeRegistry, name: &str) -> bool {
    registry.module_by_name(name).is_some()
}

pub fn has_module_with_policy(
    registry: &TypeRegistry,
    name: &str,
    _policy: &ReflectPolicy,
) -> bool {
    has_module(registry, name)
}

pub fn modules(registry: &TypeRegistry) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry.modules().map(module_record_host).collect(),
    ))
}

pub fn module_with_policy(
    registry: &TypeRegistry,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = registry.module_by_name(name).ok_or_else(|| {
        let related = module_candidates(registry, name);
        ReflectError::new(ReflectErrorKind::UnknownModule {
            module: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(module_record_with_exports(
        desc,
        visible_export_names(registry, desc, policy),
    ))
}

pub fn modules_with_policy(registry: &TypeRegistry, policy: &ReflectPolicy) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry
            .modules()
            .map(|module| {
                module_record_host_with_exports(
                    module,
                    visible_export_names(registry, module, policy),
                )
            })
            .collect(),
    ))
}

pub fn exports(registry: &TypeRegistry, module_name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.module_by_name(module_name).ok_or_else(|| {
        let related = module_candidates(registry, module_name);
        ReflectError::new(ReflectErrorKind::UnknownModule {
            module: module_name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.exports
            .iter()
            .map(|export| HostValue::String(export.name.clone()))
            .collect(),
    )))
}

pub fn exports_with_policy(
    registry: &TypeRegistry,
    module_name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = registry.module_by_name(module_name).ok_or_else(|| {
        let related = module_candidates(registry, module_name);
        ReflectError::new(ReflectErrorKind::UnknownModule {
            module: module_name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(ReflectValue::Host(HostValue::Array(
        visible_export_names(registry, desc, policy)
            .into_iter()
            .map(HostValue::String)
            .collect(),
    )))
}

pub fn function(registry: &TypeRegistry, name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.function_by_name(name).ok_or_else(|| {
        let related = function_candidates(registry, name);
        ReflectError::new(ReflectErrorKind::UnknownFunction {
            function: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(function_record(desc))
}

pub fn has_function(registry: &TypeRegistry, name: &str) -> bool {
    registry.function_by_name(name).is_some()
}

pub fn has_function_with_policy(
    registry: &TypeRegistry,
    name: &str,
    policy: &ReflectPolicy,
) -> bool {
    registry
        .function_by_name(name)
        .is_some_and(|desc| policy.require_function_access(desc).is_ok())
}

pub fn functions(registry: &TypeRegistry) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry.functions().map(function_record_host).collect(),
    ))
}

pub fn function_with_policy(
    registry: &TypeRegistry,
    name: &str,
    policy: &ReflectPolicy,
) -> ReflectResult<ReflectValue> {
    let desc = registry.function_by_name(name).ok_or_else(|| {
        let related = function_candidates(registry, name);
        ReflectError::new(ReflectErrorKind::UnknownFunction {
            function: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    policy.require_function_access(desc)?;
    Ok(function_record(desc))
}

pub fn functions_with_policy(registry: &TypeRegistry, policy: &ReflectPolicy) -> ReflectValue {
    ReflectValue::Host(HostValue::Array(
        registry
            .functions()
            .filter(|function| policy.require_function_access(function).is_ok())
            .map(function_record_host)
            .collect(),
    ))
}

fn module_candidates(registry: &TypeRegistry, name: &str) -> Vec<crate::ReflectCandidate> {
    ranked_candidates(
        name,
        registry
            .modules()
            .map(|module| (module.name.as_str(), module.source_span)),
    )
}

fn function_candidates(registry: &TypeRegistry, name: &str) -> Vec<crate::ReflectCandidate> {
    ranked_candidates(
        name,
        registry
            .functions()
            .map(|function| (function.name.as_str(), function.source_span)),
    )
}

fn module_record(desc: &ModuleDesc) -> ReflectValue {
    ReflectValue::Host(module_record_host(desc))
}

fn module_record_host(desc: &ModuleDesc) -> HostValue {
    module_record_host_with_exports(desc, desc.exports.iter().map(|export| export.name.clone()))
}

fn module_record_with_exports(
    desc: &ModuleDesc,
    exports: impl IntoIterator<Item = String>,
) -> ReflectValue {
    ReflectValue::Host(module_record_host_with_exports(desc, exports))
}

fn module_record_host_with_exports(
    desc: &ModuleDesc,
    exports: impl IntoIterator<Item = String>,
) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("name".to_owned(), HostValue::String(desc.name.clone()));
    fields.insert(
        "exports".to_owned(),
        HostValue::Array(exports.into_iter().map(HostValue::String).collect()),
    );
    fields.insert("attrs".to_owned(), attrs_value(&desc.attrs));
    fields.insert("source_span".to_owned(), span_value(desc.source_span));
    HostValue::Record {
        type_name: "ReflectModule".to_owned(),
        fields,
    }
}

fn visible_export_names(
    registry: &TypeRegistry,
    desc: &ModuleDesc,
    policy: &ReflectPolicy,
) -> Vec<String> {
    desc.exports
        .iter()
        .filter(|export| {
            let Some(function_id) = export.function else {
                return true;
            };
            registry
                .function_by_id(function_id)
                .is_some_and(|function| policy.require_function_access(function).is_ok())
        })
        .map(|export| export.name.clone())
        .collect()
}

fn function_record(desc: &FunctionDesc) -> ReflectValue {
    ReflectValue::Host(function_record_host(desc))
}

fn function_record_host(desc: &FunctionDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert(
        "id".to_owned(),
        HostValue::Int(i64::try_from(desc.id.get()).unwrap_or(i64::MAX)),
    );
    fields.insert("name".to_owned(), HostValue::String(desc.name.clone()));
    fields.insert(
        "module".to_owned(),
        desc.module
            .as_ref()
            .map_or(HostValue::Null, |module| HostValue::String(module.clone())),
    );
    fields.insert("public".to_owned(), HostValue::Bool(desc.public));
    fields.insert("effects".to_owned(), function_effects_record(desc));
    fields.insert("access".to_owned(), function_access_record(desc));
    fields.insert(
        "origin".to_owned(),
        HostValue::String(
            match desc.origin {
                DeclOrigin::Host => "host",
                DeclOrigin::Script => "script",
            }
            .to_owned(),
        ),
    );
    fields.insert(
        "return".to_owned(),
        desc.return_type
            .as_ref()
            .map_or(HostValue::Null, |return_type| {
                HostValue::String(return_type.clone())
            }),
    );
    fields.insert(
        "params".to_owned(),
        HostValue::Array(desc.params.iter().map(param_record).collect()),
    );
    fields.insert("docs".to_owned(), docs_value(desc.docs.as_deref()));
    fields.insert("attrs".to_owned(), attrs_value(&desc.attrs));
    fields.insert("source_span".to_owned(), span_value(desc.source_span));
    HostValue::Record {
        type_name: "ReflectFunction".to_owned(),
        fields,
    }
}

fn function_effects_record(desc: &FunctionDesc) -> HostValue {
    HostValue::Record {
        type_name: "ReflectEffectSet".to_owned(),
        fields: BTreeMap::from([
            (
                "reads_host".to_owned(),
                HostValue::Bool(desc.effects.reads_host),
            ),
            (
                "writes_host".to_owned(),
                HostValue::Bool(desc.effects.writes_host),
            ),
            (
                "emits_events".to_owned(),
                HostValue::Bool(desc.effects.emits_events),
            ),
        ]),
    }
}

fn function_access_record(desc: &FunctionDesc) -> HostValue {
    HostValue::Record {
        type_name: "ReflectFunctionAccess".to_owned(),
        fields: BTreeMap::from([
            ("public".to_owned(), HostValue::Bool(desc.access.public)),
            (
                "reflect_visible".to_owned(),
                HostValue::Bool(desc.access.reflect_visible),
            ),
            (
                "required_permissions".to_owned(),
                HostValue::Array(
                    desc.access
                        .required_permissions()
                        .iter()
                        .map(|permission| HostValue::String(permission.clone()))
                        .collect(),
                ),
            ),
        ]),
    }
}

fn param_record(param: &FunctionParamDesc) -> HostValue {
    let mut fields = BTreeMap::new();
    fields.insert("name".to_owned(), HostValue::String(param.name.clone()));
    fields.insert(
        "type".to_owned(),
        param
            .type_hint
            .as_ref()
            .map_or(HostValue::Null, |hint| HostValue::String(hint.clone())),
    );
    fields.insert("defaulted".to_owned(), HostValue::Bool(param.has_default));
    HostValue::Record {
        type_name: "ReflectParam".to_owned(),
        fields,
    }
}

fn apply_signature(mut desc: FunctionDesc, signature: &FunctionSignature) -> FunctionDesc {
    for param in &signature.params {
        let mut param_desc = FunctionParamDesc::new(param.name.clone())
            .defaulted(param.default_value_span.is_some());
        if let Some(type_hint) = &param.type_hint {
            param_desc = param_desc.type_hint(type_hint.display());
        }
        desc = desc.param(param_desc);
    }
    if let Some(return_type) = &signature.return_type {
        desc = desc.return_type(return_type.display());
    }
    desc
}

fn apply_function_attrs(mut desc: FunctionDesc, attrs: &[vela_hir::HirAttribute]) -> FunctionDesc {
    let reflected = ReflectedScriptAttrs::from_hir(attrs);
    desc.attrs = reflected.attrs;
    desc.docs = reflected.docs;
    desc
}

fn qualified_function_name(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_owned()
    } else {
        format!("{module}.{name}")
    }
}

fn stable_function_id(module: &str, name: &str) -> FunctionId {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for byte in b"function"
        .iter()
        .copied()
        .chain([0])
        .chain(module.bytes())
        .chain([0])
        .chain(name.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    FunctionId::new(if hash == 0 { 1 } else { hash })
}

#[cfg(test)]
mod tests {
    use vela_common::{SourceId, Span};
    use vela_hir::{ModuleGraph, ModulePath, ModuleSource};

    use super::*;

    #[test]
    fn registers_script_module_functions_and_exports() {
        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.reward"),
            r#"
pub fn grant(player: Player, amount: int = 1) -> bool {
    return true;
}

#[doc("Helper docs.")]
#[event("reward.helper")]
fn helper() {
    return null;
}
"#,
        ));
        let mut registry = TypeRegistry::new();

        registry.register_script_modules(&graph);

        let module = registry
            .module_by_name("game.reward")
            .expect("script module metadata");
        assert_eq!(module.exports.len(), 2);
        assert_eq!(module.exports[0].name, "game.reward.grant");
        assert_eq!(module.exports[0].kind, ModuleExportKind::Function);
        assert_eq!(
            module.source_span.map(|span| span.source),
            Some(SourceId::new(1))
        );

        let grant = registry
            .function_by_name("game.reward.grant")
            .expect("grant function metadata");
        assert_eq!(grant.module.as_deref(), Some("game.reward"));
        assert!(grant.public);
        assert_eq!(grant.origin, DeclOrigin::Script);
        assert_eq!(grant.params[0].name, "player");
        assert_eq!(grant.params[0].type_hint.as_deref(), Some("Player"));
        assert_eq!(grant.params[1].name, "amount");
        assert_eq!(grant.params[1].type_hint.as_deref(), Some("int"));
        assert!(grant.params[1].has_default);
        assert_eq!(grant.return_type.as_deref(), Some("bool"));
        assert_eq!(
            grant.source_span.map(|span| span.source),
            Some(SourceId::new(1))
        );

        let helper = registry
            .function_by_name("game.reward.helper")
            .expect("helper function metadata");
        assert!(!helper.public);
        assert_eq!(helper.docs.as_deref(), Some("Helper docs."));
        assert_eq!(helper.attrs.get("event"), Some("reward.helper"));
    }

    #[test]
    fn module_function_queries_return_records_and_candidates() {
        let mut registry = TypeRegistry::new();
        let function_id = FunctionId::new(7);
        let module_span = Span::new(SourceId::new(7), 10, 20);
        let function_span = Span::new(SourceId::new(7), 30, 50);
        registry.register_module(
            ModuleDesc::new("game.reward")
                .attr("domain", "gameplay")
                .source_span(module_span),
        );
        registry.register_function(
            FunctionDesc::new(function_id, "game.reward.grant")
                .module("game.reward")
                .param(
                    FunctionParamDesc::new("amount")
                        .type_hint("int")
                        .defaulted(true),
                )
                .return_type("bool")
                .effects(FunctionEffectSet::host_write())
                .access(FunctionAccess::new().require_permission("reward.grant"))
                .origin(DeclOrigin::Script)
                .docs("Grant reward.")
                .attr("event", "reward")
                .source_span(function_span),
        );

        assert!(has_module(&registry, "game.reward"));
        assert!(!has_module(&registry, "game.missing"));
        assert!(has_function(&registry, "game.reward.grant"));
        assert!(!has_function(&registry, "game.reward.missing"));

        let ReflectValue::Host(HostValue::Record {
            type_name,
            fields: module_metadata,
        }) = module(&registry, "game.reward").expect("module")
        else {
            panic!("module metadata should be a record");
        };
        assert_eq!(type_name, "ReflectModule");
        assert_eq!(
            module_metadata.get("name"),
            Some(&HostValue::String("game.reward".into()))
        );
        assert_eq!(
            module_metadata.get("attrs"),
            Some(&HostValue::Map(BTreeMap::from([(
                "domain".to_owned(),
                HostValue::String("gameplay".to_owned())
            )])))
        );
        assert_eq!(
            module_metadata.get("source_span"),
            Some(&span_value(Some(module_span)))
        );
        assert_eq!(
            exports(&registry, "game.reward").expect("exports"),
            ReflectValue::Host(HostValue::Array(vec![HostValue::String(
                "game.reward.grant".into()
            )]))
        );
        let ReflectValue::Host(HostValue::Array(modules)) = modules(&registry) else {
            panic!("module list should be an array");
        };
        assert_eq!(modules.len(), 1);
        let HostValue::Record {
            type_name,
            fields: module_list_item,
        } = &modules[0]
        else {
            panic!("module list item should be a record");
        };
        assert_eq!(type_name, "ReflectModule");
        assert_eq!(
            module_list_item.get("name"),
            Some(&HostValue::String("game.reward".into()))
        );
        let ReflectValue::Host(HostValue::Array(functions)) = functions(&registry) else {
            panic!("function list should be an array");
        };
        assert_eq!(functions.len(), 1);
        let HostValue::Record {
            type_name,
            fields: function_list_item,
        } = &functions[0]
        else {
            panic!("function list item should be a record");
        };
        assert_eq!(type_name, "ReflectFunction");
        assert_eq!(
            function_list_item.get("name"),
            Some(&HostValue::String("game.reward.grant".into()))
        );
        assert_eq!(
            function_list_item.get("id"),
            Some(&HostValue::Int(
                i64::try_from(function_id.get()).unwrap_or(i64::MAX)
            ))
        );

        let ReflectValue::Host(HostValue::Record {
            type_name,
            fields: function_metadata,
        }) = function(&registry, "game.reward.grant").expect("function")
        else {
            panic!("function metadata should be a record");
        };
        assert_eq!(type_name, "ReflectFunction");
        assert_eq!(
            function_metadata.get("id"),
            Some(&HostValue::Int(
                i64::try_from(function_id.get()).unwrap_or(i64::MAX)
            ))
        );
        assert_eq!(
            function_metadata.get("return"),
            Some(&HostValue::String("bool".into()))
        );
        assert_eq!(
            function_metadata.get("origin"),
            Some(&HostValue::String("script".into()))
        );
        assert_eq!(
            function_metadata.get("source_span"),
            Some(&span_value(Some(function_span)))
        );
        assert_eq!(
            function_metadata.get("effects"),
            Some(&HostValue::Record {
                type_name: "ReflectEffectSet".to_owned(),
                fields: BTreeMap::from([
                    ("reads_host".to_owned(), HostValue::Bool(true)),
                    ("writes_host".to_owned(), HostValue::Bool(true)),
                    ("emits_events".to_owned(), HostValue::Bool(false)),
                ]),
            })
        );
        assert_eq!(
            function_metadata.get("access"),
            Some(&HostValue::Record {
                type_name: "ReflectFunctionAccess".to_owned(),
                fields: BTreeMap::from([
                    ("public".to_owned(), HostValue::Bool(true)),
                    ("reflect_visible".to_owned(), HostValue::Bool(true)),
                    (
                        "required_permissions".to_owned(),
                        HostValue::Array(vec![HostValue::String("reward.grant".to_owned())])
                    ),
                ]),
            })
        );
        assert_eq!(
            function_metadata.get("docs"),
            Some(&HostValue::String("Grant reward.".into()))
        );
        assert_eq!(
            function_metadata.get("attrs"),
            Some(&HostValue::Map(BTreeMap::from([(
                "event".to_owned(),
                HostValue::String("reward".to_owned())
            )])))
        );

        let error = module(&registry, "game.rewards").expect_err("unknown module");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownModule {
                module: "game.rewards".to_owned(),
                candidates: vec!["game.reward".to_owned()],
                related: vec![crate::ReflectCandidate::new(
                    "game.reward",
                    Some(module_span)
                )],
            }
        );

        let error = function(&registry, "game.reward.grnat").expect_err("unknown function");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownFunction {
                function: "game.reward.grnat".to_owned(),
                candidates: vec!["game.reward.grant".to_owned()],
                related: vec![crate::ReflectCandidate::new(
                    "game.reward.grant",
                    Some(function_span)
                )],
            }
        );
    }

    #[test]
    fn function_policy_rejects_hidden_private_and_unapproved_functions() {
        let mut registry = TypeRegistry::new();
        registry.register_function(
            FunctionDesc::new(FunctionId::new(1), "game.hidden")
                .access(FunctionAccess::new().reflect_visible(false)),
        );
        registry.register_function(
            FunctionDesc::new(FunctionId::new(2), "game.private")
                .access(FunctionAccess::new().public(false).reflect_visible(true)),
        );
        registry.register_function(
            FunctionDesc::new(FunctionId::new(3), "game.admin")
                .access(FunctionAccess::new().require_permission("game.admin")),
        );
        let private_policy = ReflectPolicy::new(
            crate::ReflectPermissionSet::new().with(crate::ReflectPermission::AccessPrivate),
        );

        let error = function_with_policy(&registry, "game.hidden", &ReflectPolicy::all())
            .expect_err("hidden function");
        assert_eq!(
            error.kind,
            ReflectErrorKind::FunctionNotReflectVisible {
                function: "game.hidden".to_owned()
            }
        );

        let error = function_with_policy(&registry, "game.private", &ReflectPolicy::read_only())
            .expect_err("private function");
        assert_eq!(
            error.kind,
            ReflectErrorKind::PermissionDenied {
                permission: crate::ReflectPermission::AccessPrivate
            }
        );

        let error = function_with_policy(&registry, "game.admin", &private_policy)
            .expect_err("missing function permission");
        assert_eq!(
            error.kind,
            ReflectErrorKind::FunctionPermissionDenied {
                function: "game.admin".to_owned(),
                permission: "game.admin".to_owned()
            }
        );
    }

    #[test]
    fn function_policy_allows_private_functions_with_permissions() {
        let mut registry = TypeRegistry::new();
        registry.register_function(
            FunctionDesc::new(FunctionId::new(1), "game.private_admin").access(
                FunctionAccess::new()
                    .public(false)
                    .reflect_visible(true)
                    .require_permission("game.admin"),
            ),
        );
        let policy = ReflectPolicy::new(
            crate::ReflectPermissionSet::new().with(crate::ReflectPermission::AccessPrivate),
        )
        .with_function_permission("game.admin");

        let ReflectValue::Host(HostValue::Record {
            fields: function, ..
        }) = function_with_policy(&registry, "game.private_admin", &policy)
            .expect("private function metadata")
        else {
            panic!("function metadata should be a record");
        };

        assert_eq!(function.get("public"), Some(&HostValue::Bool(false)));
    }

    #[test]
    fn module_exports_with_policy_hide_inaccessible_functions() {
        let mut registry = TypeRegistry::new();
        registry.register_module(ModuleDesc::new("game.reward"));
        registry.register_function(
            FunctionDesc::new(FunctionId::new(1), "game.reward.grant").module("game.reward"),
        );
        registry.register_function(
            FunctionDesc::new(FunctionId::new(2), "game.reward.hidden")
                .module("game.reward")
                .access(FunctionAccess::new().reflect_visible(false)),
        );
        registry.register_function(
            FunctionDesc::new(FunctionId::new(3), "game.reward.private")
                .module("game.reward")
                .access(FunctionAccess::new().public(false).reflect_visible(true)),
        );
        registry.register_function(
            FunctionDesc::new(FunctionId::new(4), "game.reward.admin")
                .module("game.reward")
                .access(FunctionAccess::new().require_permission("game.admin")),
        );

        assert!(has_module_with_policy(
            &registry,
            "game.reward",
            &ReflectPolicy::read_only()
        ));
        assert!(!has_module_with_policy(
            &registry,
            "game.missing",
            &ReflectPolicy::read_only()
        ));
        assert!(has_function_with_policy(
            &registry,
            "game.reward.grant",
            &ReflectPolicy::read_only()
        ));
        assert!(!has_function_with_policy(
            &registry,
            "game.reward.hidden",
            &ReflectPolicy::read_only()
        ));
        assert!(!has_function_with_policy(
            &registry,
            "game.reward.private",
            &ReflectPolicy::read_only()
        ));
        assert!(!has_function_with_policy(
            &registry,
            "game.reward.admin",
            &ReflectPolicy::read_only()
        ));

        assert_eq!(
            exports(&registry, "game.reward").expect("raw exports"),
            ReflectValue::Host(HostValue::Array(vec![
                HostValue::String("game.reward.grant".to_owned()),
                HostValue::String("game.reward.hidden".to_owned()),
                HostValue::String("game.reward.private".to_owned()),
                HostValue::String("game.reward.admin".to_owned()),
            ]))
        );
        let ReflectValue::Host(HostValue::Array(raw_modules)) = modules(&registry) else {
            panic!("raw module list should be an array");
        };
        assert_eq!(raw_modules.len(), 1);
        let ReflectValue::Host(HostValue::Array(raw_functions)) = functions(&registry) else {
            panic!("raw function list should be an array");
        };
        assert_eq!(raw_functions.len(), 4);
        assert_eq!(
            exports_with_policy(&registry, "game.reward", &ReflectPolicy::read_only())
                .expect("policy exports"),
            ReflectValue::Host(HostValue::Array(vec![HostValue::String(
                "game.reward.grant".to_owned()
            )]))
        );
        let ReflectValue::Host(HostValue::Array(policy_functions)) =
            functions_with_policy(&registry, &ReflectPolicy::read_only())
        else {
            panic!("policy function list should be an array");
        };
        assert_eq!(policy_functions.len(), 1);
        let ReflectValue::Host(HostValue::Array(policy_modules)) =
            modules_with_policy(&registry, &ReflectPolicy::read_only())
        else {
            panic!("policy module list should be an array");
        };
        let HostValue::Record {
            fields: policy_module,
            ..
        } = &policy_modules[0]
        else {
            panic!("policy module list item should be a record");
        };
        assert_eq!(
            policy_module.get("exports"),
            Some(&HostValue::Array(vec![HostValue::String(
                "game.reward.grant".to_owned()
            )]))
        );

        let ReflectValue::Host(HostValue::Record { fields: module, .. }) =
            module_with_policy(&registry, "game.reward", &ReflectPolicy::read_only())
                .expect("policy module")
        else {
            panic!("module metadata should be a record");
        };
        assert_eq!(
            module.get("exports"),
            Some(&HostValue::Array(vec![HostValue::String(
                "game.reward.grant".to_owned()
            )]))
        );

        let admin_policy = ReflectPolicy::new(
            crate::ReflectPermissionSet::read_only().with(crate::ReflectPermission::AccessPrivate),
        )
        .with_function_permission("game.admin");
        assert!(has_function_with_policy(
            &registry,
            "game.reward.admin",
            &admin_policy
        ));
        assert_eq!(
            exports_with_policy(&registry, "game.reward", &admin_policy).expect("admin exports"),
            ReflectValue::Host(HostValue::Array(vec![
                HostValue::String("game.reward.grant".to_owned()),
                HostValue::String("game.reward.private".to_owned()),
                HostValue::String("game.reward.admin".to_owned()),
            ]))
        );
    }
}
