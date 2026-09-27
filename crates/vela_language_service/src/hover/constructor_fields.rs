use vela_analysis::type_fact::TypeFact;
use vela_hir::{
    module_graph::{Declaration, DeclarationKind, Visibility},
    type_hint::EnumVariantFieldsHint,
};
use vela_syntax::{
    SyntaxToken,
    ast::{AstNode, SyntaxCallExpr, SyntaxRecordExpr, SyntaxRecordPattern},
};

use super::{Hover, HoverKind};
use crate::{
    DiagnosticRange, DisplayParts, LanguageServiceDatabases, QueryContext,
    symbol_target::SymbolTarget,
};

#[derive(Clone, Copy)]
enum Form {
    Record,
    Tuple,
}

pub(super) fn hover(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Option<Hover>> {
    let tree = query.syntax_parse()?.tree();
    let matches = |token: SyntaxToken| {
        usize::from(token.text_range().start()) == target.range().start
            && usize::from(token.text_range().end()) == target.range().end
    };
    for node in tree.syntax().descendants() {
        let site = if let Some(record) = SyntaxRecordExpr::cast(node.clone()) {
            record
                .fields()
                .iter()
                .any(|field| !field.is_shorthand() && field.label_token().is_some_and(&matches))
                .then(|| (record.path_segments(), Form::Record))
        } else if let Some(pattern) = SyntaxRecordPattern::cast(node.clone()) {
            pattern
                .fields()
                .any(|field| !field.is_shorthand() && field.label_token().is_some_and(&matches))
                .then(|| (pattern.path_segments(), Form::Record))
        } else if let Some(call) = SyntaxCallExpr::cast(node) {
            call.arguments()
                .iter()
                .any(|argument| argument.name_token().is_some_and(&matches))
                .then(|| {
                    call.callee()?
                        .as_path()
                        .map(|path| (path.path_segments(), Form::Tuple))
                })
                .flatten()
        } else {
            None
        };
        let Some((path, form)) = site else {
            continue;
        };
        let graph = db.hir_db().graph();
        let module = graph.module_id(query.module_key()?)?;
        // A bare record constructor uses the declaration namespace in HIR.
        // Qualified heads and ordinary calls still respect lexical values.
        let expanded = if matches!(form, Form::Record) && path.len() == 1 {
            graph.expand_import_path(module, &path)
        } else {
            query.expand_import_path(&path)
        };
        let Some(path) = expanded else {
            return Some(None);
        };
        let (name, parent) = path.split_last()?;
        if matches!(form, Form::Record)
            && let Some(declaration) =
                graph.resolve_visible_declaration_path(module, &path, DeclarationKind::Struct)
        {
            if declaration.module != module && declaration.visibility != Visibility::Public {
                return Some(None);
            }
            return Some(
                graph
                    .struct_shape(declaration.id)?
                    .fields
                    .iter()
                    .find(|field| field.name == target.text())
                    .map(|field| super::struct_field_hover(graph, declaration, field, range)),
            );
        }
        if let Some(declaration) =
            graph.resolve_visible_declaration_path(module, parent, DeclarationKind::Enum)
        {
            if declaration.module != module && declaration.visibility != Visibility::Public {
                return Some(None);
            }
            return Some(variant_field_hover(
                db,
                declaration,
                name,
                target.text(),
                form,
                range,
            ));
        }
        // Ordinary callable argument names belong to S5. Source constructor
        // labels, including inaccessible or wrong-kind owners, own null.
        if matches!(form, Form::Tuple) {
            return None;
        }
        let key = graph.module_key(module)?;
        if [
            DeclarationKind::Struct,
            DeclarationKind::Enum,
            DeclarationKind::Trait,
            DeclarationKind::Function,
            DeclarationKind::Const,
            DeclarationKind::State,
        ]
        .into_iter()
        .any(|kind| {
            graph.declaration_by_type_path(&path, key, kind).is_some()
                || graph.declaration_by_type_path(parent, key, kind).is_some()
        }) {
            return Some(None);
        }
        let schema = db.schema_db().facts();
        let owner = path.join("::");
        let known = matches!(
            schema.type_fact(&owner),
            Some(TypeFact::Host { .. } | TypeFact::Record { .. })
        ) || (matches!(
            schema.type_fact(&parent.join("::")),
            Some(TypeFact::Enum { .. })
        ) && schema.variant_fact(&parent.join("::"), name).is_some());
        return Some(
            (known && schema.field_fact(&owner, target.text()).is_some())
                .then(|| {
                    super::schema::member_hover(
                        schema,
                        &TypeFact::record(owner),
                        target.text(),
                        range,
                    )
                })
                .flatten(),
        );
    }
    None
}

pub(super) fn member_hover(
    db: &LanguageServiceDatabases,
    receiver: &TypeFact,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Option<Hover>> {
    let TypeFact::Enum { name, variant } = receiver else {
        return None;
    };
    let graph = db.hir_db().graph();
    if let Some(declaration) = target
        .member_receiver_declaration()
        .and_then(|id| graph.declaration(id))
        .filter(|declaration| declaration.kind == DeclarationKind::Enum)
    {
        return variant
            .as_ref()
            .and_then(|variant| {
                variant_field_hover(db, declaration, variant, target.text(), Form::Record, range)
            })
            .map(Some);
    }
    let variant = variant.as_ref()?;
    let schema = db.schema_db().facts();
    schema.variant_fact(name, variant)?;
    let owner = format!("{name}::{variant}");
    schema.field_fact(&owner, target.text())?;
    Some(super::schema::member_hover(
        schema,
        &TypeFact::record(owner),
        target.text(),
        range,
    ))
}

fn variant_field_hover(
    db: &LanguageServiceDatabases,
    declaration: &Declaration,
    variant: &str,
    name: &str,
    form: Form,
    range: DiagnosticRange,
) -> Option<Hover> {
    let graph = db.hir_db().graph();
    let variant = graph
        .enum_shape(declaration.id)?
        .variants
        .iter()
        .find(|entry| entry.name == variant)?;
    let (hint, docs) = match (&variant.fields, form) {
        (EnumVariantFieldsHint::Record(fields), Form::Record) => {
            let field = fields.iter().find(|field| field.name == name)?;
            (field.type_hint.as_ref(), super::attr_docs(&field.attrs))
        }
        (EnumVariantFieldsHint::Tuple(fields), Form::Tuple) => {
            let field = fields.iter().find(|field| field.name == name)?;
            (field.type_hint.as_ref(), None)
        }
        _ => return None,
    };
    Some(Hover::new(
        range,
        format!(
            "{}::{}.{}",
            super::qualified_declaration_label(graph, declaration),
            variant.name,
            name
        ),
        HoverKind::Field,
        DisplayParts::type_name(hint.map_or_else(|| "Any".to_owned(), |hint| hint.display())),
        docs,
        crate::symbol_ref::source_variant_field_symbol(graph, declaration.id, &variant.name, name),
    ))
}
