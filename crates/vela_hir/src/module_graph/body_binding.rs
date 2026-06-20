use std::collections::BTreeMap;

use vela_common::Diagnostic;

use crate::binding::{BindingMap, ImportBinding, SyntaxFunctionBindingInput, bind_syntax_function};
use crate::ids::{HirDeclId, HirNodeId, ModuleId};
use crate::module_graph::{HirModule, ModuleGraph};
use crate::type_hint::ParamHint;

use super::model::ImportResolution;
use super::names::import_binding_name;
use super::syntax_summary::SyntaxBodySourceParts;

#[derive(Clone, Debug)]
pub(super) struct FunctionBodySource {
    declaration: HirDeclId,
    params: Vec<ParamHint>,
    syntax: SyntaxBodySourceParts,
}

impl FunctionBodySource {
    pub(super) fn new(
        declaration: HirDeclId,
        params: Vec<ParamHint>,
        syntax: SyntaxBodySourceParts,
    ) -> Self {
        Self {
            declaration,
            params,
            syntax,
        }
    }
}

impl ModuleGraph {
    pub(super) fn bind_function_body(&mut self, module: &HirModule, source: FunctionBodySource) {
        let declaration = source.declaration;
        let (bindings, diagnostics) = self.bind_body(module, source);
        self.bindings.insert(declaration, bindings);
        self.diagnostics.extend(diagnostics);
    }

    pub(super) fn bind_trait_default_method_body(
        &mut self,
        module: &HirModule,
        method: HirNodeId,
        source: FunctionBodySource,
    ) {
        let (bindings, diagnostics) = self.bind_body(module, source);
        self.trait_default_method_bindings.insert(method, bindings);
        self.diagnostics.extend(diagnostics);
    }

    pub(super) fn bind_impl_method_body(
        &mut self,
        module: &HirModule,
        method: HirNodeId,
        source: FunctionBodySource,
    ) {
        let (bindings, diagnostics) = self.bind_body(module, source);
        self.impl_method_bindings.insert(method, bindings);
        self.diagnostics.extend(diagnostics);
    }

    fn bind_body(
        &mut self,
        module: &HirModule,
        source: FunctionBodySource,
    ) -> (BindingMap, Vec<Diagnostic>) {
        let module_declarations = module
            .declarations
            .names()
            .filter_map(|name| {
                module
                    .declarations
                    .get(name)
                    .map(|declaration| (name.to_owned(), declaration))
            })
            .collect::<Vec<_>>();
        let imports = self.import_bindings(module);
        let qualified_declarations = self.qualified_declarations_with(module);

        bind_syntax_function(SyntaxFunctionBindingInput {
            source: module.source,
            declaration: source.declaration,
            params: &source.params,
            default_params: source.syntax.default_params,
            body: source.syntax.body,
            module_declarations,
            qualified_declarations,
            imports,
            next_expr_id: &mut self.next_expr_id,
            next_local_id: &mut self.next_local_id,
        })
    }

    fn import_bindings(&self, module: &HirModule) -> Vec<ImportBinding> {
        module
            .imports
            .iter()
            .filter_map(|import| {
                let name = import_binding_name(import)?;
                let declaration = match import.resolution {
                    Some(ImportResolution::Declaration(declaration)) => Some(declaration),
                    None => self.lookup_import_declaration(import.module, &import.path),
                };
                Some(ImportBinding { name, declaration })
            })
            .collect()
    }

    fn qualified_declarations_with(&self, current: &HirModule) -> Vec<(Vec<String>, HirDeclId)> {
        let mut declarations = self.qualified_declarations_for(current.id);
        declarations.extend(self.qualified_declarations_in(current, current.id));
        declarations.into_iter().collect()
    }

    pub(super) fn qualified_declarations_for(
        &self,
        requesting_module: ModuleId,
    ) -> BTreeMap<Vec<String>, HirDeclId> {
        self.modules
            .iter()
            .flat_map(|module| self.qualified_declarations_in(module, requesting_module))
            .collect()
    }

    fn qualified_declarations_in(
        &self,
        module: &HirModule,
        requesting_module: ModuleId,
    ) -> Vec<(Vec<String>, HirDeclId)> {
        module
            .declarations
            .names()
            .filter_map(|name| {
                let declaration = module.declarations.get(name)?;
                if !self.declaration_visible_from(declaration, requesting_module) {
                    return None;
                }
                let mut path = module.path.segments().to_vec();
                path.push(name.to_owned());
                Some((path, declaration))
            })
            .collect()
    }
}
