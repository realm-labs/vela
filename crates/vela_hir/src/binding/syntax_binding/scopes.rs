use std::collections::BTreeMap;

use vela_common::{Diagnostic, Span};

use super::{ActiveScope, SyntaxBindingLowerer};
use crate::binding::{LocalBinding, LocalBindingKind};
use crate::body::{HirScope, HirScopeKind, HirSourceOrigin};
use crate::ids::{HirLocalId, HirScopeId};
use crate::type_hint::HirTypeHint;

impl SyntaxBindingLowerer<'_> {
    pub(super) fn declare_local(
        &mut self,
        name: String,
        kind: LocalBindingKind,
        type_hint: Option<HirTypeHint>,
        span: Span,
    ) -> HirLocalId {
        self.declare_local_with_scope(name, kind, type_hint, span, None)
    }

    pub(super) fn declare_pattern_local(
        &mut self,
        name: String,
        kind: LocalBindingKind,
        span: Span,
        scope_span: Span,
    ) -> HirLocalId {
        self.declare_local_with_scope(name, kind, None, span, Some(scope_span))
    }

    pub(super) fn declare_local_with_scope(
        &mut self,
        name: String,
        kind: LocalBindingKind,
        type_hint: Option<HirTypeHint>,
        span: Span,
        scope_span: Option<Span>,
    ) -> HirLocalId {
        let id = self.next_local();
        self.scopes
            .last_mut()
            .expect("function binding always has a scope")
            .locals
            .insert(name.clone(), id);
        self.locals_by_name
            .entry(name.clone())
            .or_default()
            .push(id);
        self.locals.insert(
            id,
            LocalBinding {
                id,
                name,
                kind,
                type_hint,
                span,
                scope_span,
            },
        );
        let body = self.current_body();
        let scope = self
            .scopes
            .last()
            .expect("function binding always has a scope")
            .id;
        self.local_bodies.insert(id, body);
        self.body_mut(body).locals.push(id);
        if let Some(scope) = self.body_mut(body).scopes.get_mut(&scope) {
            scope.locals.push(id);
        }
        id
    }

    pub(super) fn declare_parameter(
        &mut self,
        name: String,
        kind: LocalBindingKind,
        type_hint: Option<HirTypeHint>,
        span: Span,
    ) -> HirLocalId {
        if let Some(previous) = self
            .scopes
            .last()
            .and_then(|scope| scope.locals.get(&name))
            .and_then(|local| self.locals.get(local))
        {
            self.diagnostics.push(
                Diagnostic::error(format!("duplicate parameter `{name}`"))
                    .with_code("hir::duplicate_parameter")
                    .with_span(span)
                    .with_label(previous.span, "previous parameter is here")
                    .with_label(span, "duplicate parameter is here"),
            );
        }
        let local = self.declare_local(name, kind, type_hint, span);
        if self
            .locals
            .get(&local)
            .is_some_and(|binding| binding.name == "self")
        {
            self.body_mut(self.current_body()).self_binding = Some(local);
        }
        self.next_param(local, span);
        local
    }

    pub(super) fn push_scope(&mut self, kind: HirScopeKind, span: Span) {
        let id = HirScopeId::new(*self.next_scope_id);
        *self.next_scope_id = self.next_scope_id.saturating_add(1);
        let body = self.current_body();
        let parent = self.scopes.last().map(|scope| scope.id);
        let source = self.source;
        self.body_mut(body).scopes.insert(
            id,
            HirScope {
                id,
                parent,
                origin: HirSourceOrigin { source, span },
                kind,
                locals: Vec::new(),
                children: Vec::new(),
            },
        );
        if let Some(parent) = parent
            && let Some(parent_scope) = self.body_mut(body).scopes.get_mut(&parent)
        {
            parent_scope.children.push(id);
        }
        self.scopes.push(ActiveScope {
            id,
            locals: BTreeMap::new(),
        });
    }

    pub(super) fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn next_local(&mut self) -> HirLocalId {
        let id = HirLocalId::new(*self.next_local_id);
        *self.next_local_id = self.next_local_id.saturating_add(1);
        id
    }
}
