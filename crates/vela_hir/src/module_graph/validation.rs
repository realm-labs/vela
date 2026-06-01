use std::collections::BTreeMap;

use vela_common::{Diagnostic, Span};
use vela_syntax::ast::{EnumItem, EnumVariantFields, ImplItem, StructItem, TraitItem};

use super::names::import_binding_name;
use super::*;

impl ModuleGraph {
    pub(super) fn validate_import_bindings(&mut self, module: &HirModule) {
        let mut imported_names = BTreeMap::new();
        for import in &module.imports {
            let Some(name) = import_binding_name(import) else {
                continue;
            };
            if let Some(previous_span) = imported_names.insert(name.clone(), import.span) {
                self.diagnostics.push(
                    Diagnostic::error(format!("duplicate import `{name}`"))
                        .with_code("hir::duplicate_import")
                        .with_span(import.span)
                        .with_label(previous_span, "previous import is here")
                        .with_label(import.span, "duplicate import is here"),
                );
            }
            if let Some(declaration) = module
                .declarations
                .get(&name)
                .and_then(|declaration| self.declarations.get(&declaration))
            {
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "import `{name}` conflicts with a local declaration"
                    ))
                    .with_code("hir::import_conflict")
                    .with_span(import.span)
                    .with_label(declaration.span, "local declaration is here")
                    .with_label(import.span, "conflicting import is here"),
                );
            }
        }
    }

    pub(super) fn validate_struct_shape(&mut self, item: &StructItem) {
        self.validate_member_names(
            &item.fields,
            |field| (&field.name, field.span),
            "field",
            "hir::duplicate_field",
        );
    }

    pub(super) fn validate_enum_shape(&mut self, item: &EnumItem) {
        self.validate_member_names(
            &item.variants,
            |variant| (&variant.name, variant.span),
            "variant",
            "hir::duplicate_variant",
        );
        for variant in &item.variants {
            match &variant.fields {
                EnumVariantFields::Unit => {}
                EnumVariantFields::Tuple(params) => {
                    self.validate_member_names(
                        params,
                        |param| (&param.name, param.span),
                        "variant field",
                        "hir::duplicate_variant_field",
                    );
                }
                EnumVariantFields::Record(fields) => {
                    self.validate_member_names(
                        fields,
                        |field| (&field.name, field.span),
                        "variant field",
                        "hir::duplicate_variant_field",
                    );
                }
            }
        }
    }

    pub(super) fn validate_trait_shape(&mut self, item: &TraitItem) {
        self.validate_member_names(
            &item.methods,
            |method| (&method.name, method.span),
            "trait method",
            "hir::duplicate_trait_method",
        );
        for method in &item.methods {
            self.validate_member_names(
                &method.params,
                |param| (&param.name, param.span),
                "parameter",
                "hir::duplicate_parameter",
            );
        }
    }

    pub(super) fn validate_impl_shape(&mut self, item: &ImplItem) {
        self.validate_member_names(
            &item.methods,
            |method| (&method.function.name, method.function.body.span),
            "impl method",
            "hir::duplicate_impl_method",
        );
    }

    fn validate_member_names<T>(
        &mut self,
        members: &[T],
        member_name: impl Fn(&T) -> (&String, Span),
        label: &str,
        code: &'static str,
    ) {
        let mut names = BTreeMap::new();
        for member in members {
            let (name, span) = member_name(member);
            if let Some(previous_span) = names.insert(name.clone(), span) {
                self.diagnostics.push(
                    Diagnostic::error(format!("duplicate {label} `{name}`"))
                        .with_code(code)
                        .with_span(span)
                        .with_label(previous_span, format!("previous {label} is here"))
                        .with_label(span, format!("duplicate {label} is here")),
                );
            }
        }
    }
}
