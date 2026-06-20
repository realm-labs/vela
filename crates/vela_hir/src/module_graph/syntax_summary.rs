use vela_common::{SourceId, Span};
use vela_syntax::ast::{
    AstNode, SyntaxConstItem, SyntaxEnumItem, SyntaxFunctionItem, SyntaxGlobalItem, SyntaxImplItem,
    SyntaxItem, SyntaxSourceFile, SyntaxStructItem, SyntaxTraitItem, SyntaxUseItem, Visibility,
};
use vela_syntax::{Parse as SyntaxParse, SyntaxKind, TextRange};

use super::model::DeclarationKind;
use super::names::{inherent_impl_declaration_name, trait_impl_declaration_name};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SyntaxModuleSummary {
    module_span: Span,
    item_headers: Vec<SyntaxItemHeader>,
}

impl SyntaxModuleSummary {
    pub(super) fn from_parse(source: SourceId, parsed: &SyntaxParse<SyntaxSourceFile>) -> Self {
        let item_headers = parsed
            .tree()
            .items()
            .filter_map(|item| SyntaxItemHeader::from_item(source, &item))
            .collect::<Vec<_>>();
        let module_span = item_headers
            .first()
            .map_or_else(|| Span::new(source, 0, 0), SyntaxItemHeader::span);
        Self {
            module_span,
            item_headers,
        }
    }

    pub(super) const fn module_span(&self) -> Span {
        self.module_span
    }

    pub(super) fn import_or(
        &self,
        index: usize,
        fallback_path: &[String],
        fallback_alias: &Option<String>,
        fallback_span: Span,
    ) -> (Vec<String>, Option<String>, Span) {
        match self.item_headers.get(index) {
            Some(SyntaxItemHeader::Import { path, alias, span }) => {
                (path.clone(), alias.clone(), *span)
            }
            _ => (
                fallback_path.to_vec(),
                fallback_alias.clone(),
                fallback_span,
            ),
        }
    }

    pub(super) fn declaration_or(
        &self,
        index: usize,
        kind: DeclarationKind,
        fallback_name: String,
        fallback_visibility: Visibility,
        fallback_span: Span,
    ) -> (String, Visibility, Span) {
        match self.item_headers.get(index) {
            Some(SyntaxItemHeader::Declaration {
                kind: header_kind,
                name,
                visibility,
                span,
            }) if *header_kind == kind => (name.clone(), visibility.clone(), *span),
            _ => (fallback_name, fallback_visibility, fallback_span),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SyntaxItemHeader {
    Import {
        path: Vec<String>,
        alias: Option<String>,
        span: Span,
    },
    Declaration {
        kind: DeclarationKind,
        name: String,
        visibility: Visibility,
        span: Span,
    },
}

impl SyntaxItemHeader {
    fn from_item(source: SourceId, item: &SyntaxItem) -> Option<Self> {
        match item.syntax().kind() {
            SyntaxKind::UseItem => {
                let use_item = SyntaxUseItem::cast(item.syntax().clone())?;
                Some(Self::Import {
                    path: use_item
                        .path()
                        .map(|path| path.path_segments())
                        .unwrap_or_default(),
                    alias: use_item.alias_text(),
                    span: span_for(source, item.text_range()),
                })
            }
            SyntaxKind::ConstItem => {
                let item = SyntaxConstItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Const,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::GlobalItem => {
                let item = SyntaxGlobalItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Global,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::FunctionItem => {
                let item = SyntaxFunctionItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Function,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::StructItem => {
                let item = SyntaxStructItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Struct,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::EnumItem => {
                let item = SyntaxEnumItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Enum,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::TraitItem => {
                let item = SyntaxTraitItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Trait,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::ImplItem => {
                let item = SyntaxImplItem::cast(item.syntax().clone())?;
                let target_path = item.target_path_segments();
                let trait_path = item.trait_path_segments();
                let name = if trait_path.is_empty() {
                    inherent_impl_declaration_name(&target_path)
                } else {
                    trait_impl_declaration_name(&trait_path, &target_path)
                };
                Some(Self::Declaration {
                    kind: DeclarationKind::Impl,
                    name,
                    visibility: visibility(item.is_public()),
                    span: span_for(source, item.syntax().text_range()),
                })
            }
            _ => None,
        }
    }

    const fn span(&self) -> Span {
        match self {
            Self::Import { span, .. } | Self::Declaration { span, .. } => *span,
        }
    }
}

fn declaration_header(
    source: SourceId,
    range: TextRange,
    kind: DeclarationKind,
    name: Option<String>,
    is_public: bool,
) -> Option<SyntaxItemHeader> {
    Some(SyntaxItemHeader::Declaration {
        kind,
        name: name?,
        visibility: visibility(is_public),
        span: span_for(source, range),
    })
}

fn visibility(is_public: bool) -> Visibility {
    if is_public {
        Visibility::Public
    } else {
        Visibility::Private
    }
}

fn span_for(source: SourceId, range: TextRange) -> Span {
    Span::new(source, range.start().into(), range.end().into())
}
