use std::collections::BTreeMap;

use vela_common::Span;

use super::{ActiveScope, SyntaxBindingLowerer};
use crate::binding::{BindingResolution, ExprInfo};
use crate::body::{
    HirBlock, HirBody, HirBodyOwner, HirCapture, HirExpr, HirExprKind, HirParam, HirPattern,
    HirPatternKind, HirScope, HirScopeKind, HirSourceOrigin, HirStmt, HirStmtKind,
};
use crate::ids::{
    HirBlockId, HirBodyId, HirCaptureId, HirExprId, HirLocalId, HirParamId, HirPatternId,
    HirScopeId, HirStmtId,
};

impl<'a> SyntaxBindingLowerer<'a> {
    pub(super) fn current_body(&self) -> HirBodyId {
        *self
            .body_stack
            .last()
            .expect("body lowering always has an active body")
    }

    pub(super) fn body_mut(&mut self, body: HirBodyId) -> &mut HirBody {
        self.bodies
            .get_mut(&body)
            .expect("allocated HIR body should be stored")
    }

    pub(super) fn with_body(&mut self, body: HirBodyId, f: impl FnOnce(&mut Self)) {
        self.body_stack.push(body);
        if let Some(root_scope) = self.body_mut(body).root_scope {
            self.scopes.push(ActiveScope {
                id: root_scope,
                locals: BTreeMap::new(),
            });
        }
        f(self);
        if self.body_mut(body).root_scope.is_some_and(|root_scope| {
            self.scopes
                .last()
                .is_some_and(|scope| scope.id == root_scope)
        }) {
            self.scopes.pop();
        }
        self.body_stack.pop();
    }

    pub(super) fn next_body(&mut self, owner: HirBodyOwner, span: Span) -> HirBodyId {
        let id = HirBodyId::new(*self.next_body_id);
        *self.next_body_id = self.next_body_id.saturating_add(1);
        let root_scope = HirScopeId::new(*self.next_scope_id);
        *self.next_scope_id = self.next_scope_id.saturating_add(1);
        let source = self.source;
        let mut body = HirBody::new(id, owner, HirSourceOrigin { source, span });
        body.root_scope = Some(root_scope);
        body.scopes.insert(
            root_scope,
            HirScope {
                id: root_scope,
                parent: None,
                origin: HirSourceOrigin { source, span },
                kind: HirScopeKind::Body,
                locals: Vec::new(),
                children: Vec::new(),
            },
        );
        self.bodies.insert(id, body);
        id
    }

    pub(super) fn next_block(&mut self, span: Span) -> HirBlockId {
        let id = HirBlockId::new(*self.next_block_id);
        *self.next_block_id = self.next_block_id.saturating_add(1);
        let body = self.current_body();
        let source = self.source;
        self.body_mut(body).blocks.insert(
            id,
            HirBlock {
                id,
                origin: HirSourceOrigin { source, span },
                statements: Vec::new(),
            },
        );
        id
    }

    pub(super) fn next_stmt(&mut self, span: Span, kind: HirStmtKind) -> HirStmtId {
        let id = HirStmtId::new(*self.next_stmt_id);
        *self.next_stmt_id = self.next_stmt_id.saturating_add(1);
        let body = self.current_body();
        let source = self.source;
        self.body_mut(body).statements.insert(
            id,
            HirStmt {
                id,
                origin: HirSourceOrigin { source, span },
                kind,
            },
        );
        if let Some(block) = self.block_stack.last().copied()
            && let Some(block) = self.body_mut(body).blocks.get_mut(&block)
        {
            block.statements.push(id);
        }
        id
    }

    pub(super) fn next_expr(&mut self, span: Span, kind: HirExprKind) -> HirExprId {
        let id = HirExprId::new(*self.next_expr_id);
        *self.next_expr_id = self.next_expr_id.saturating_add(1);
        self.expressions.insert(id, ExprInfo { id, span });
        let body = self.current_body();
        let source = self.source;
        self.body_mut(body).expressions.insert(
            id,
            HirExpr {
                id,
                origin: HirSourceOrigin { source, span },
                kind,
            },
        );
        id
    }

    pub(super) fn next_pattern(&mut self, span: Span, kind: HirPatternKind) -> HirPatternId {
        let id = HirPatternId::new(*self.next_pattern_id);
        *self.next_pattern_id = self.next_pattern_id.saturating_add(1);
        let body = self.current_body();
        let source = self.source;
        self.body_mut(body).patterns.insert(
            id,
            HirPattern {
                id,
                origin: HirSourceOrigin { source, span },
                kind,
                local: None,
            },
        );
        id
    }

    pub(super) fn next_param(&mut self, local: HirLocalId, span: Span) -> HirParamId {
        let id = HirParamId::new(*self.next_param_id);
        *self.next_param_id = self.next_param_id.saturating_add(1);
        let body = self.current_body();
        let source = self.source;
        self.body_mut(body).params.push(HirParam {
            id,
            local,
            origin: HirSourceOrigin { source, span },
            default_body: None,
        });
        id
    }

    pub(super) fn next_capture(
        &mut self,
        owner: HirBodyId,
        local: HirLocalId,
        use_expression: HirExprId,
    ) {
        if self.capture_keys.contains_key(&(owner, local)) {
            return;
        }
        let id = HirCaptureId::new(*self.next_capture_id);
        *self.next_capture_id = self.next_capture_id.saturating_add(1);
        self.capture_keys.insert((owner, local), id);
        self.body_mut(owner).captures.push(HirCapture {
            id,
            local,
            use_expression,
            owner,
        });
    }

    pub(super) fn record_capture_for_resolution(
        &mut self,
        expression: HirExprId,
        resolution: &BindingResolution,
    ) {
        let BindingResolution::Local(local) = resolution else {
            return;
        };
        let current_body = self.current_body();
        if self.local_bodies.get(local).copied() != Some(current_body) {
            self.next_capture(current_body, *local, expression);
        }
    }
}
