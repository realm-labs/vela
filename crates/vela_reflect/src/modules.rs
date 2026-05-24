use std::collections::BTreeMap;

use vela_common::FunctionId;
use vela_hir::{DeclarationKind, FunctionSignature, ModuleGraph};
use vela_host::HostValue;
use vela_syntax::Visibility;

use crate::{
    AttrMap, ReflectError, ReflectErrorKind, ReflectResult, ReflectValue, TypeRegistry,
    metadata::{attrs_value, docs_value},
    name_candidates,
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
    pub origin: DeclOrigin,
    pub docs: Option<String>,
    pub attrs: AttrMap,
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
            origin: DeclOrigin::Host,
            docs: None,
            attrs: AttrMap::new(),
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
}

impl ModuleDesc {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            exports: Vec::new(),
            attrs: AttrMap::new(),
        }
    }

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value);
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
                self.register_module(ModuleDesc::new(module_name));
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
            .origin(DeclOrigin::Script);
            if let Some(signature) = signature {
                desc = apply_signature(desc, signature);
            }
            self.register_function(desc);
        }
    }
}

pub fn module(registry: &TypeRegistry, name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.module_by_name(name).ok_or_else(|| {
        ReflectError::new(ReflectErrorKind::UnknownModule {
            module: name.to_owned(),
            candidates: name_candidates(
                name,
                registry.modules().map(|module| module.name.as_str()),
            ),
        })
    })?;
    Ok(module_record(desc))
}

pub fn exports(registry: &TypeRegistry, module_name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.module_by_name(module_name).ok_or_else(|| {
        ReflectError::new(ReflectErrorKind::UnknownModule {
            module: module_name.to_owned(),
            candidates: name_candidates(
                module_name,
                registry.modules().map(|module| module.name.as_str()),
            ),
        })
    })?;
    Ok(ReflectValue::Host(HostValue::Array(
        desc.exports
            .iter()
            .map(|export| HostValue::String(export.name.clone()))
            .collect(),
    )))
}

pub fn function(registry: &TypeRegistry, name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.function_by_name(name).ok_or_else(|| {
        ReflectError::new(ReflectErrorKind::UnknownFunction {
            function: name.to_owned(),
            candidates: name_candidates(
                name,
                registry.functions().map(|function| function.name.as_str()),
            ),
        })
    })?;
    Ok(function_record(desc))
}

fn module_record(desc: &ModuleDesc) -> ReflectValue {
    let mut fields = BTreeMap::new();
    fields.insert(
        "name".to_owned(),
        ReflectValue::Host(HostValue::String(desc.name.clone())),
    );
    fields.insert(
        "exports".to_owned(),
        ReflectValue::Host(HostValue::Array(
            desc.exports
                .iter()
                .map(|export| HostValue::String(export.name.clone()))
                .collect(),
        )),
    );
    fields.insert(
        "attrs".to_owned(),
        ReflectValue::Host(attrs_value(&desc.attrs)),
    );
    ReflectValue::Record(fields)
}

fn function_record(desc: &FunctionDesc) -> ReflectValue {
    let mut fields = BTreeMap::new();
    fields.insert(
        "name".to_owned(),
        ReflectValue::Host(HostValue::String(desc.name.clone())),
    );
    fields.insert(
        "module".to_owned(),
        ReflectValue::Host(
            desc.module
                .as_ref()
                .map_or(HostValue::Null, |module| HostValue::String(module.clone())),
        ),
    );
    fields.insert(
        "public".to_owned(),
        ReflectValue::Host(HostValue::Bool(desc.public)),
    );
    fields.insert(
        "origin".to_owned(),
        ReflectValue::Host(HostValue::String(
            match desc.origin {
                DeclOrigin::Host => "host",
                DeclOrigin::Script => "script",
            }
            .to_owned(),
        )),
    );
    fields.insert(
        "return".to_owned(),
        ReflectValue::Host(
            desc.return_type
                .as_ref()
                .map_or(HostValue::Null, |return_type| {
                    HostValue::String(return_type.clone())
                }),
        ),
    );
    fields.insert(
        "params".to_owned(),
        ReflectValue::Host(HostValue::Array(
            desc.params.iter().map(param_record).collect(),
        )),
    );
    fields.insert(
        "docs".to_owned(),
        ReflectValue::Host(docs_value(desc.docs.as_deref())),
    );
    fields.insert(
        "attrs".to_owned(),
        ReflectValue::Host(attrs_value(&desc.attrs)),
    );
    ReflectValue::Record(fields)
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
    use vela_common::SourceId;
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

        let helper = registry
            .function_by_name("game.reward.helper")
            .expect("helper function metadata");
        assert!(!helper.public);
    }

    #[test]
    fn module_function_queries_return_records_and_candidates() {
        let mut registry = TypeRegistry::new();
        let function_id = FunctionId::new(7);
        registry.register_module(ModuleDesc::new("game.reward").attr("domain", "gameplay"));
        registry.register_function(
            FunctionDesc::new(function_id, "game.reward.grant")
                .module("game.reward")
                .param(
                    FunctionParamDesc::new("amount")
                        .type_hint("int")
                        .defaulted(true),
                )
                .return_type("bool")
                .origin(DeclOrigin::Script)
                .docs("Grant reward.")
                .attr("event", "reward"),
        );

        let ReflectValue::Record(module_metadata) =
            module(&registry, "game.reward").expect("module")
        else {
            panic!("module metadata should be a record");
        };
        assert_eq!(
            module_metadata.get("name"),
            Some(&ReflectValue::Host(HostValue::String("game.reward".into())))
        );
        assert_eq!(
            module_metadata.get("attrs"),
            Some(&ReflectValue::Host(HostValue::Map(BTreeMap::from([(
                "domain".to_owned(),
                HostValue::String("gameplay".to_owned())
            )]))))
        );
        assert_eq!(
            exports(&registry, "game.reward").expect("exports"),
            ReflectValue::Host(HostValue::Array(vec![HostValue::String(
                "game.reward.grant".into()
            )]))
        );

        let ReflectValue::Record(function) =
            function(&registry, "game.reward.grant").expect("function")
        else {
            panic!("function metadata should be a record");
        };
        assert_eq!(
            function.get("return"),
            Some(&ReflectValue::Host(HostValue::String("bool".into())))
        );
        assert_eq!(
            function.get("origin"),
            Some(&ReflectValue::Host(HostValue::String("script".into())))
        );
        assert_eq!(
            function.get("docs"),
            Some(&ReflectValue::Host(HostValue::String(
                "Grant reward.".into()
            )))
        );
        assert_eq!(
            function.get("attrs"),
            Some(&ReflectValue::Host(HostValue::Map(BTreeMap::from([(
                "event".to_owned(),
                HostValue::String("reward".to_owned())
            )]))))
        );

        let error = module(&registry, "game.rewards").expect_err("unknown module");
        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownModule {
                module: "game.rewards".to_owned(),
                candidates: vec!["game.reward".to_owned()]
            }
        );
    }
}
