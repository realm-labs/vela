use std::collections::BTreeMap;

use vela_common::{Diagnostic, Span};

use super::names::import_binding_name;
use super::*;
use crate::attributes::{HirAttribute, SchemaIdAttrError, parse_schema_id_attr};
use crate::type_hint::{EnumShape, EnumVariantFieldsHint, ImplMetadata, StructShape, TraitShape};

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

    pub(super) fn validate_struct_shape(&mut self, shape: &StructShape) {
        self.validate_member_names(
            &shape.fields,
            |field| (&field.name, field.span),
            "field",
            "hir::duplicate_field",
        );
        self.validate_member_schema_ids(
            &shape.fields,
            |field| (&field.name, field.span, field.attrs.as_slice()),
            "field",
            "hir::duplicate_field_id",
        );
    }

    pub(super) fn validate_enum_shape(&mut self, shape: &EnumShape) {
        self.validate_member_names(
            &shape.variants,
            |variant| (&variant.name, variant.span),
            "variant",
            "hir::duplicate_variant",
        );
        self.validate_member_schema_ids(
            &shape.variants,
            |variant| (&variant.name, variant.span, variant.attrs.as_slice()),
            "variant",
            "hir::duplicate_variant_id",
        );
        for variant in &shape.variants {
            match &variant.fields {
                EnumVariantFieldsHint::Unit => {}
                EnumVariantFieldsHint::Tuple(params) => {
                    self.validate_member_names(
                        params,
                        |param| (&param.name, param.span),
                        "variant field",
                        "hir::duplicate_variant_field",
                    );
                }
                EnumVariantFieldsHint::Record(fields) => {
                    self.validate_member_names(
                        fields,
                        |field| (&field.name, field.span),
                        "variant field",
                        "hir::duplicate_variant_field",
                    );
                    self.validate_member_schema_ids(
                        fields,
                        |field| (&field.name, field.span, field.attrs.as_slice()),
                        "variant field",
                        "hir::duplicate_variant_field_id",
                    );
                }
            }
        }
    }

    pub(super) fn validate_trait_shape(&mut self, shape: &TraitShape) {
        self.validate_member_names(
            &shape.methods,
            |method| (&method.name, method.span),
            "trait method",
            "hir::duplicate_trait_method",
        );
        for method in &shape.methods {
            self.validate_member_names(
                &method.signature.params,
                |param| (&param.name, param.span),
                "parameter",
                "hir::duplicate_parameter",
            );
        }
    }

    pub(super) fn validate_impl_shape(&mut self, metadata: &ImplMetadata) {
        self.validate_member_names(
            &metadata.methods,
            |method| (&method.name, method.span),
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

    fn validate_member_schema_ids<T>(
        &mut self,
        members: &[T],
        member_info: impl Fn(&T) -> (&String, Span, &[HirAttribute]),
        label: &str,
        code: &'static str,
    ) {
        let mut ids = BTreeMap::new();
        for member in members {
            let (name, span, attrs) = member_info(member);
            let Some((id, id_span)) = self.member_schema_id(name, attrs) else {
                continue;
            };
            if let Some((previous_name, previous_span)) = ids.insert(id, (name.clone(), span)) {
                self.diagnostics.push(
                    Diagnostic::error(format!("duplicate {label} id {id}"))
                        .with_code(code)
                        .with_span(id_span)
                        .with_label(
                            previous_span,
                            format!("previous {label} `{previous_name}` uses this id"),
                        )
                        .with_label(span, format!("duplicate {label} `{name}` uses this id")),
                );
            }
        }
    }

    fn member_schema_id(
        &mut self,
        member_name: &str,
        attrs: &[HirAttribute],
    ) -> Option<(u64, Span)> {
        let mut found = None;
        for attr in attrs {
            match parse_schema_id_attr(&attr.name, attr.value.as_deref()) {
                Ok(Some(id)) => {
                    if let Some((previous_id, previous_span)) = found {
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "duplicate id attribute on schema member `{member_name}`"
                            ))
                            .with_code("hir::duplicate_schema_id_attr")
                            .with_span(attr.span)
                            .with_label(previous_span, format!("previous id {previous_id} is here"))
                            .with_label(attr.span, format!("duplicate id {id} is here")),
                        );
                        continue;
                    }
                    found = Some((id, attr.span));
                }
                Ok(None) => {}
                Err(error) => {
                    self.diagnostics
                        .push(schema_id_attr_diagnostic(member_name, attr.span, error));
                }
            }
        }
        found
    }
}

fn schema_id_attr_diagnostic(
    member_name: &str,
    span: Span,
    error: SchemaIdAttrError,
) -> Diagnostic {
    let reason = match error {
        SchemaIdAttrError::MissingValue => "missing id value",
        SchemaIdAttrError::InvalidValue => "id value must be a u64 integer",
        SchemaIdAttrError::Zero => "id value must be non-zero",
    };
    Diagnostic::error(format!(
        "invalid id attribute on schema member `{member_name}`"
    ))
    .with_code("hir::invalid_schema_id")
    .with_span(span)
    .with_label(span, reason)
}
