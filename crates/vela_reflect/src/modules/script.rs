use vela_def::script_function_id;
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
            let qualified_name = graph
                .qualified_declaration_name(declaration.id)
                .expect("stored script function has a module path");
            let signature = graph.function_signature(declaration.id);
            let mut desc = FunctionDesc::new(script_function_id(&qualified_name), qualified_name)
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
