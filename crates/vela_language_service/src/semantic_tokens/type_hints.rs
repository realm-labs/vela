//! Precise type-path spans and provenance in the requesting module's scope.
use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_hir::{
    ids::ModuleId,
    module_graph::{DeclarationKind, Visibility},
};
use vela_syntax::{
    SyntaxKind,
    ast::{AstNode, SyntaxTypeHint},
};

use super::{SemanticTokenClassification, SemanticTokenModifiers as M, SemanticTokenType as T};
use crate::LanguageServiceDatabases;

pub(super) fn collect(
    db: &LanguageServiceDatabases,
    source_id: SourceId,
) -> BTreeMap<(usize, usize), SemanticTokenClassification> {
    let mut result = BTreeMap::new();
    let Some(source) = db
        .source_db()
        .records()
        .values()
        .find(|source| source.source_id() == source_id)
    else {
        return result;
    };
    let Some(parse) = db.parse_db().syntax_parse(source.document_id()) else {
        return result;
    };
    let Some(module) = db.hir_db().graph().module_id(source.module_key()) else {
        return result;
    };
    for hint in parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxTypeHint::cast)
    {
        let path = hint.path_segments();
        let tokens = hint
            .path_tokens()
            .into_iter()
            .filter(|token| token.kind() == SyntaxKind::Ident)
            .collect::<Vec<_>>();
        let builtin = is_builtin(&path);
        let modifiers = if builtin {
            M::BUILTIN
        } else {
            provenance(db, module, &path)
        };
        for (index, token) in tokens.iter().enumerate() {
            let terminal = index + 1 == tokens.len();
            let kind = if !terminal {
                T::Module
            } else if builtin {
                T::BuiltinType
            } else {
                T::Type
            };
            result.insert(
                (
                    usize::from(token.text_range().start()),
                    usize::from(token.text_range().end()),
                ),
                SemanticTokenClassification::new(kind, if terminal { modifiers } else { M::NONE }),
            );
        }
    }
    result
}

fn provenance(db: &LanguageServiceDatabases, module: ModuleId, path: &[String]) -> M {
    let graph = db.hir_db().graph();
    let Some(path) = graph.expand_import_path(module, path) else {
        return M::NONE;
    };
    let Some(current) = graph.module_key(module) else {
        return M::NONE;
    };
    for kind in [
        DeclarationKind::Struct,
        DeclarationKind::Enum,
        DeclarationKind::Trait,
        DeclarationKind::Function,
        DeclarationKind::Const,
        DeclarationKind::State,
    ] {
        if let Some(declaration) = graph.declaration_by_type_path(&path, current, kind) {
            return if matches!(
                kind,
                DeclarationKind::Struct | DeclarationKind::Enum | DeclarationKind::Trait
            ) && (declaration.module == module
                || declaration.visibility == Visibility::Public)
            {
                M::SOURCE
            } else {
                M::NONE
            };
        }
    }
    let schema = db.schema_db().facts();
    let name = path.join("::");
    if schema.type_fact(&name).is_some() || schema.trait_fact(&name).is_some() {
        M::HOST.union(M::SCHEMA)
    } else {
        M::NONE
    }
}

fn is_builtin(path: &[String]) -> bool {
    let [name] = path else {
        return false;
    };
    matches!(
        name.as_str(),
        "Any"
            | "String"
            | "Bytes"
            | "Function"
            | "Closure"
            | "Range"
            | "Iterator"
            | "Array"
            | "ArrayView"
            | "ArrayMut"
            | "Map"
            | "MapView"
            | "MapMut"
            | "Set"
            | "SetView"
            | "SetMut"
            | "Option"
            | "Result"
            | "bool"
            | "char"
            | "f32"
            | "f64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
    )
}
