use vela_analysis::{fact_scope::ExprFactScope, type_fact::TypeFact};
use vela_common::Span;
use vela_syntax::ast::SyntaxPattern;

use super::ExpressionFactCollector;

impl ExpressionFactCollector<'_> {
    pub(super) fn insert_pattern_facts(
        &mut self,
        scope: &mut ExprFactScope,
        pattern: &SyntaxPattern,
        fact: &TypeFact,
    ) {
        if let Some(name_token) = pattern.binding_name_token() {
            let span = Span::new(
                self.source,
                u32::from(name_token.text_range().start()),
                u32::from(name_token.text_range().end()),
            );
            if let Some(pattern) = self.graph.pattern_at_span(span) {
                self.pattern_facts.insert(pattern, fact.clone());
            }
            scope.insert_path([name_token.text().to_owned()], fact.clone());
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
            self.insert_pattern_facts(scope, pattern, element);
        }
    }
}

pub(super) fn insert_pattern_scope_facts(
    scope: &mut ExprFactScope,
    pattern: &SyntaxPattern,
    fact: &TypeFact,
) {
    if let Some(name) = pattern.binding_name() {
        scope.insert_path([name], fact.clone());
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
        insert_pattern_scope_facts(scope, pattern, element);
    }
}
