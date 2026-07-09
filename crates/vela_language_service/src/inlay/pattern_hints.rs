use vela_analysis::type_fact::TypeFact;
use vela_syntax::SyntaxToken;
use vela_syntax::ast::SyntaxPattern;

use crate::{SymbolRef, TextRange};

use super::{InlayHint, InlayHintKind, TypeHintCollector, text_size_to_usize, type_hint_label};

impl TypeHintCollector<'_, '_> {
    pub(super) fn collect_pattern_type_hints(&mut self, pattern: &SyntaxPattern, fact: &TypeFact) {
        if let Some(name_token) = pattern.binding_name_token() {
            self.collect_binding_token_type_hint(&name_token, fact);
            return;
        }

        let Some(tuple) = pattern.tuple_pattern() else {
            return;
        };
        if tuple.path_text().is_some() {
            return;
        }
        let TypeFact::Tuple { elements } = fact else {
            return;
        };
        let patterns = tuple.patterns().collect::<Vec<_>>();
        if patterns.len() != elements.len() {
            return;
        }
        for (pattern, element) in patterns.iter().zip(elements) {
            self.collect_pattern_type_hints(pattern, element);
        }
    }

    pub(super) fn collect_binding_token_type_hint(
        &mut self,
        name_token: &SyntaxToken,
        fact: &TypeFact,
    ) {
        let Some(label) = type_hint_label(fact) else {
            return;
        };
        let position_offset = text_size_to_usize(name_token.text_range().end());
        if self.range.contains(position_offset) {
            let start = text_size_to_usize(name_token.text_range().start());
            self.hints.push(InlayHint {
                position: self.line_index.position(position_offset),
                label,
                kind: InlayHintKind::Type,
                symbol: Some(SymbolRef::local_at(
                    name_token.text().to_owned(),
                    self.document_id.clone(),
                    TextRange::new(start, position_offset),
                )),
            });
        }
    }
}

pub(super) fn iterable_item_fact(fact: &TypeFact) -> Option<TypeFact> {
    match fact {
        TypeFact::Array { element } | TypeFact::Iterator { item: element } => {
            Some((**element).clone())
        }
        TypeFact::Union(facts) => {
            let item = TypeFact::union(facts.iter().filter_map(iterable_item_fact));
            (!matches!(item, TypeFact::Unknown)).then_some(item)
        }
        _ => None,
    }
}
