use vela_common::FunctionId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::FunctionSignature;
use vela_syntax::ast::Visibility;

use crate::{registry::TypeRegistry, script_attrs::ReflectedScriptAttrs};

use super::descriptors::{DeclOrigin, FunctionDesc, FunctionParamDesc, ModuleDesc};

impl TypeRegistry {
    pub fn register_script_modules(&mut self, graph: &ModuleGraph) {
        for declaration in graph.declarations() {
            let Some(module_name) = graph
                .module_path(declaration.module)
                .map(|path| path.join())
            else {
                continue;
            };
            if !module_name.is_empty() && self.module_by_name(&module_name).is_none() {
                self.register_module(
                    ModuleDesc::new(module_name)
                        .origin(DeclOrigin::Script)
                        .source_span(declaration.span),
                );
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
            .public(declaration.visibility == Visibility::Public)
            .origin(DeclOrigin::Script)
            .source_span(declaration.span);
            if !module_name.is_empty() {
                desc = desc.module(module_name);
            }
            if let Some(signature) = signature {
                desc = apply_signature(desc, signature);
            }
            desc = apply_function_attrs(desc, graph.declaration_attrs(declaration.id));
            self.register_function(desc);
        }
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

fn apply_function_attrs(
    mut desc: FunctionDesc,
    attrs: &[vela_hir::attributes::HirAttribute],
) -> FunctionDesc {
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
