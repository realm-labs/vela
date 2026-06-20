use std::collections::BTreeSet;

use vela_hir::module_graph::{ModulePath, stable_source_hash};
use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::{
    AstNode, SyntaxConstItem, SyntaxEnumItem, SyntaxEnumVariant, SyntaxFunctionItem,
    SyntaxGlobalItem, SyntaxImplItem, SyntaxImplMethod, SyntaxItem, SyntaxParam, SyntaxSourceFile,
    SyntaxStructField, SyntaxStructItem, SyntaxTraitItem, SyntaxTraitMethod, SyntaxTypeHint,
    SyntaxUseItem,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ParseSummary {
    pub(super) imports: BTreeSet<ModulePath>,
    pub(super) declaration_fingerprint: u64,
    pub(super) import_fingerprint: u64,
}

pub(super) fn summarize_source(parsed: &SyntaxParse<SyntaxSourceFile>) -> ParseSummary {
    let mut declarations = Vec::new();
    let mut imports = BTreeSet::new();
    let mut import_fingerprint_parts = Vec::new();

    let tree = parsed.tree();
    for item in tree.items() {
        match item.syntax().kind() {
            vela_syntax::SyntaxKind::UseItem => {
                let Some(use_item) = SyntaxUseItem::cast(item.syntax().clone()) else {
                    continue;
                };
                let path = use_item
                    .path()
                    .map(|path| path.path_segments())
                    .unwrap_or_default();
                if let Some((_, module_segments)) = path.split_last() {
                    imports.insert(ModulePath::new(module_segments.iter().cloned()));
                }
                import_fingerprint_parts.push(format!(
                    "use:{} as {}",
                    path.join("::"),
                    use_item.alias_text().as_deref().unwrap_or("")
                ));
            }
            vela_syntax::SyntaxKind::ConstItem => {
                let Some(inner) = SyntaxConstItem::cast(item.syntax().clone()) else {
                    continue;
                };
                declarations.push(format!(
                    "{} const {}:{}",
                    syntax_visibility(&item),
                    inner.name_text().unwrap_or_default(),
                    optional_syntax_hint(inner.type_hint())
                ));
            }
            vela_syntax::SyntaxKind::GlobalItem => {
                let Some(inner) = SyntaxGlobalItem::cast(item.syntax().clone()) else {
                    continue;
                };
                declarations.push(format!(
                    "{} global {}:{}",
                    syntax_visibility(&item),
                    inner.name_text().unwrap_or_default(),
                    optional_syntax_hint(inner.type_hint())
                ));
            }
            vela_syntax::SyntaxKind::FunctionItem => {
                let Some(function) = SyntaxFunctionItem::cast(item.syntax().clone()) else {
                    continue;
                };
                declarations.push(format!(
                    "{} {}",
                    syntax_visibility(&item),
                    syntax_function_signature(&function)
                ));
            }
            vela_syntax::SyntaxKind::StructItem => {
                let Some(inner) = SyntaxStructItem::cast(item.syntax().clone()) else {
                    continue;
                };
                declarations.push(format!(
                    "{} struct {} {}",
                    syntax_visibility(&item),
                    inner.name_text().unwrap_or_default(),
                    syntax_fields_signature(inner.field_list().map(|list| list.fields()))
                ));
            }
            vela_syntax::SyntaxKind::EnumItem => {
                let Some(inner) = SyntaxEnumItem::cast(item.syntax().clone()) else {
                    continue;
                };
                let variants = inner
                    .variant_list()
                    .into_iter()
                    .flat_map(|list| list.variants())
                    .map(|variant| {
                        let fields = syntax_variant_fields_signature(&variant);
                        format!("{}({fields})", variant.name_text().unwrap_or_default())
                    })
                    .collect::<Vec<_>>()
                    .join("|");
                declarations.push(format!(
                    "{} enum {} {}",
                    syntax_visibility(&item),
                    inner.name_text().unwrap_or_default(),
                    variants
                ));
            }
            vela_syntax::SyntaxKind::TraitItem => {
                let Some(inner) = SyntaxTraitItem::cast(item.syntax().clone()) else {
                    continue;
                };
                declarations.push(format!(
                    "{} {}",
                    syntax_visibility(&item),
                    syntax_trait_signature(&inner)
                ));
            }
            vela_syntax::SyntaxKind::ImplItem => {
                let Some(inner) = SyntaxImplItem::cast(item.syntax().clone()) else {
                    continue;
                };
                declarations.push(format!(
                    "{} {}",
                    syntax_visibility(&item),
                    syntax_impl_signature(&inner)
                ));
            }
            _ => {}
        }
    }
    declarations.sort();
    import_fingerprint_parts.sort();
    ParseSummary {
        imports,
        declaration_fingerprint: stable_source_hash(&declarations.join("\n")),
        import_fingerprint: stable_source_hash(&import_fingerprint_parts.join("\n")),
    }
}

fn syntax_visibility(item: &SyntaxItem) -> &'static str {
    if item.is_public() {
        "public"
    } else {
        "private"
    }
}

fn syntax_function_signature(function: &SyntaxFunctionItem) -> String {
    format!(
        "fn {}({}) -> {}",
        function.name_text().unwrap_or_default(),
        syntax_params_signature(function.param_list().map(|list| list.params())),
        optional_syntax_hint(function.return_type())
    )
}

fn syntax_trait_signature(item: &SyntaxTraitItem) -> String {
    format!(
        "trait {} {}",
        item.name_text().unwrap_or_default(),
        item.methods()
            .map(|method| syntax_trait_method_signature(&method))
            .collect::<Vec<_>>()
            .join("|")
    )
}

fn syntax_trait_method_signature(method: &SyntaxTraitMethod) -> String {
    format!(
        "{}({}) -> {} default:{}",
        method.name_text().unwrap_or_default(),
        syntax_params_signature(method.param_list().map(|list| list.params())),
        optional_syntax_hint(method.return_type()),
        method.body().is_some()
    )
}

fn syntax_impl_signature(item: &SyntaxImplItem) -> String {
    let owner = item.target_path_segments().join("::");
    let trait_path = item.trait_path_segments();
    let kind = if trait_path.is_empty() {
        "impl".to_owned()
    } else {
        format!("impl {}", trait_path.join("::"))
    };
    format!(
        "{kind} for {owner} {}",
        item.methods()
            .map(|method| syntax_impl_method_signature(&method))
            .collect::<Vec<_>>()
            .join("|")
    )
}

fn syntax_impl_method_signature(method: &SyntaxImplMethod) -> String {
    format!(
        "fn {}({}) -> {}",
        method.name_text().unwrap_or_default(),
        syntax_params_signature(method.param_list().map(|list| list.params())),
        optional_syntax_hint(method.return_type())
    )
}

fn syntax_params_signature(params: Option<impl Iterator<Item = SyntaxParam>>) -> String {
    params
        .into_iter()
        .flatten()
        .map(|param| {
            format!(
                "{}:{}={}",
                param.name_text().unwrap_or_default(),
                optional_syntax_hint(param.type_hint()),
                param.default_value().is_some()
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn syntax_fields_signature(fields: Option<impl Iterator<Item = SyntaxStructField>>) -> String {
    fields
        .into_iter()
        .flatten()
        .map(|field| {
            format!(
                "{}:{}={}",
                field.name_text().unwrap_or_default(),
                optional_syntax_hint(field.type_hint()),
                field.default_value().is_some()
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn syntax_variant_fields_signature(variant: &SyntaxEnumVariant) -> String {
    if let Some(fields) = variant.tuple_field_list() {
        syntax_params_signature(Some(fields.params()))
    } else if let Some(fields) = variant.record_field_list() {
        syntax_fields_signature(Some(fields.fields()))
    } else {
        String::new()
    }
}

fn optional_syntax_hint(hint: Option<SyntaxTypeHint>) -> String {
    hint.as_ref()
        .map_or_else(String::new, syntax_hint_signature)
}

fn syntax_hint_signature(hint: &SyntaxTypeHint) -> String {
    let path = hint.path_segments().join("::");
    let args = hint
        .type_arg_list()
        .into_iter()
        .flat_map(|list| list.type_hints())
        .map(|arg| syntax_hint_signature(&arg))
        .collect::<Vec<_>>();
    if args.is_empty() {
        path
    } else {
        format!("{}<{}>", path, args.join(","))
    }
}
