use std::collections::BTreeMap;

use vela_common::Diagnostic;

use crate::binding::{
    BindingMap, ImportBinding, SyntaxExpressionBindingInput, SyntaxFunctionBindingInput,
    bind_syntax_expression_body, bind_syntax_function,
};
use crate::body::HirBodyOwner;
use crate::ids::{HirBodyId, HirDeclId, HirNodeId, ModuleId};
use crate::module_graph::{HirModule, ModuleGraph};
use crate::type_hint::{EnumVariantFieldsHint, ParamHint};

use super::model::ImportResolution;
use super::names::import_binding_name;
use super::syntax_summary::{
    SchemaFieldDefaultTarget, SyntaxBodySourceParts, SyntaxExpressionSourcePart,
    SyntaxSchemaFieldDefaultSource,
};

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

#[derive(Clone, Debug)]
pub(super) struct ExpressionBodySource {
    declaration: HirDeclId,
    syntax: SyntaxExpressionSourcePart,
}

impl ExpressionBodySource {
    pub(super) const fn new(declaration: HirDeclId, syntax: SyntaxExpressionSourcePart) -> Self {
        Self {
            declaration,
            syntax,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct SchemaFieldDefaultBodySource {
    declaration: HirDeclId,
    target: SchemaFieldDefaultTarget,
    syntax: SyntaxExpressionSourcePart,
}

impl SchemaFieldDefaultBodySource {
    pub(super) fn new(declaration: HirDeclId, source: SyntaxSchemaFieldDefaultSource) -> Self {
        Self {
            declaration,
            target: source.target,
            syntax: source.expression,
        }
    }
}

impl ModuleGraph {
    pub(super) fn bind_const_initializer_body(
        &mut self,
        module: &HirModule,
        source: ExpressionBodySource,
    ) {
        let declaration = source.declaration;
        let body = self.next_body_id();
        let (bindings, diagnostics) = self.bind_expression_body(
            module,
            source,
            body,
            HirBodyOwner::ConstInitializer(declaration),
        );
        self.const_initializer_bodies.insert(declaration, body);
        self.const_initializer_bindings
            .insert(declaration, bindings);
        self.diagnostics.extend(diagnostics);
    }

    pub(super) fn bind_schema_field_default_body(
        &mut self,
        module: &HirModule,
        source: SchemaFieldDefaultBodySource,
    ) {
        let declaration = source.declaration;
        let target = source.target;
        let body = self.next_body_id();
        let (bindings, diagnostics) = self.bind_expression_body(
            module,
            ExpressionBodySource::new(declaration, source.syntax),
            body,
            HirBodyOwner::SchemaFieldDefault(declaration),
        );
        self.schema_field_default_bindings.insert(body, bindings);
        self.attach_schema_field_default_body(declaration, target, body);
        self.diagnostics.extend(diagnostics);
    }

    fn attach_schema_field_default_body(
        &mut self,
        declaration: HirDeclId,
        target: SchemaFieldDefaultTarget,
        body: HirBodyId,
    ) {
        match target {
            SchemaFieldDefaultTarget::Struct { field } => {
                if let Some(field) = self
                    .struct_shapes
                    .get_mut(&declaration)
                    .and_then(|shape| shape.fields.get_mut(field))
                {
                    field.default_body = Some(body);
                }
            }
            SchemaFieldDefaultTarget::EnumTuple { variant, field } => {
                let Some(variant) = self
                    .enum_shapes
                    .get_mut(&declaration)
                    .and_then(|shape| shape.variants.get_mut(variant))
                else {
                    return;
                };
                if let EnumVariantFieldsHint::Tuple(fields) = &mut variant.fields
                    && let Some(field) = fields.get_mut(field)
                {
                    field.default_body = Some(body);
                }
            }
            SchemaFieldDefaultTarget::EnumRecord { variant, field } => {
                let Some(variant) = self
                    .enum_shapes
                    .get_mut(&declaration)
                    .and_then(|shape| shape.variants.get_mut(variant))
                else {
                    return;
                };
                if let EnumVariantFieldsHint::Record(fields) = &mut variant.fields
                    && let Some(field) = fields.get_mut(field)
                {
                    field.default_body = Some(body);
                }
            }
        }
    }

    pub(super) fn bind_function_body(&mut self, module: &HirModule, source: FunctionBodySource) {
        let declaration = source.declaration;
        let body = self.next_body_id();
        let (bindings, diagnostics) =
            self.bind_body(module, source, body, HirBodyOwner::Declaration(declaration));
        self.function_bodies.insert(declaration, body);
        self.bindings.insert(declaration, bindings);
        self.diagnostics.extend(diagnostics);
    }

    pub(super) fn bind_trait_default_method_body(
        &mut self,
        module: &HirModule,
        method: HirNodeId,
        source: FunctionBodySource,
    ) {
        let body = self.next_body_id();
        let (bindings, diagnostics) = self.bind_body(
            module,
            source,
            body,
            HirBodyOwner::TraitDefaultMethod(method),
        );
        self.trait_default_method_bodies.insert(method, body);
        self.trait_default_method_bindings.insert(method, bindings);
        self.diagnostics.extend(diagnostics);
    }

    pub(super) fn bind_impl_method_body(
        &mut self,
        module: &HirModule,
        method: HirNodeId,
        source: FunctionBodySource,
    ) {
        let body = self.next_body_id();
        let (bindings, diagnostics) =
            self.bind_body(module, source, body, HirBodyOwner::ImplMethod(method));
        self.impl_method_bodies.insert(method, body);
        self.impl_method_bindings.insert(method, bindings);
        self.diagnostics.extend(diagnostics);
    }

    fn bind_expression_body(
        &mut self,
        module: &HirModule,
        source: ExpressionBodySource,
        body: HirBodyId,
        owner: HirBodyOwner,
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

        let (bindings, bodies, diagnostics) =
            bind_syntax_expression_body(SyntaxExpressionBindingInput {
                source: module.source,
                declaration: source.declaration,
                expression: source.syntax.expression,
                module_declarations,
                qualified_declarations,
                imports,
                body_id: body,
                owner,
                next_expr_id: &mut self.next_expr_id,
                next_local_id: &mut self.next_local_id,
                next_body_id: &mut self.next_body_id,
                next_block_id: &mut self.next_block_id,
                next_match_arm_id: &mut self.next_match_arm_id,
                next_path_id: &mut self.next_path_id,
                next_scope_id: &mut self.next_scope_id,
                next_stmt_id: &mut self.next_stmt_id,
                next_pattern_id: &mut self.next_pattern_id,
                next_param_id: &mut self.next_param_id,
                next_capture_id: &mut self.next_capture_id,
            });
        self.extend_bodies(bodies);
        (bindings, diagnostics)
    }

    fn bind_body(
        &mut self,
        module: &HirModule,
        source: FunctionBodySource,
        body: HirBodyId,
        owner: HirBodyOwner,
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

        let (bindings, bodies, diagnostics) = bind_syntax_function(SyntaxFunctionBindingInput {
            source: module.source,
            declaration: source.declaration,
            params: &source.params,
            default_params: source.syntax.default_params,
            body: source.syntax.body,
            module_declarations,
            qualified_declarations,
            imports,
            body_id: body,
            owner,
            next_expr_id: &mut self.next_expr_id,
            next_local_id: &mut self.next_local_id,
            next_body_id: &mut self.next_body_id,
            next_block_id: &mut self.next_block_id,
            next_match_arm_id: &mut self.next_match_arm_id,
            next_path_id: &mut self.next_path_id,
            next_scope_id: &mut self.next_scope_id,
            next_stmt_id: &mut self.next_stmt_id,
            next_pattern_id: &mut self.next_pattern_id,
            next_param_id: &mut self.next_param_id,
            next_capture_id: &mut self.next_capture_id,
        });
        self.extend_bodies(bodies);
        (bindings, diagnostics)
    }

    fn import_bindings(&self, module: &HirModule) -> Vec<ImportBinding> {
        module
            .imports
            .iter()
            .filter_map(|import| {
                let name = import_binding_name(import)?;
                let declaration = match import.resolution {
                    Some(ImportResolution::Declaration(declaration)) => Some(declaration),
                    None => {
                        self.lookup_import_declaration(&module.key, import.module, &import.path)
                    }
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
                let mut path = module.key.path.segments().to_vec();
                path.push(name.to_owned());
                Some((path, declaration))
            })
            .collect()
    }
}
