use std::collections::BTreeMap;

use vela_common::Span;

use super::{ActiveScope, SyntaxBindingLowerer};
use crate::binding::BindingResolution;
use crate::body::{
    HirBlock, HirBody, HirBodyOwner, HirCapture, HirExpr, HirExprKind, HirLiteral, HirMatchArm,
    HirMatchArmBody, HirParam, HirPath, HirPathKind, HirPathOwner, HirPattern, HirPatternKind,
    HirScope, HirScopeKind, HirSourceOrigin, HirStmt, HirStmtKind, HirUnresolvedReference,
};
use crate::ids::{
    HirBlockId, HirBodyId, HirCaptureId, HirExprId, HirLocalId, HirMatchArmId, HirParamId,
    HirPathId, HirPatternId, HirScopeId, HirStmtId,
};

impl<'a> SyntaxBindingLowerer<'a> {
    pub(super) fn current_body(&self) -> HirBodyId {
        *self
            .body_stack
            .last()
            .expect("body lowering always has an active body")
    }

    pub(super) fn current_scope(&self) -> HirScopeId {
        self.scopes
            .last()
            .expect("body lowering always has an active scope")
            .id
    }

    pub(super) fn body_mut(&mut self, body: HirBodyId) -> &mut HirBody {
        self.bodies
            .get_mut(&body)
            .expect("allocated HIR body should be stored")
    }

    pub(super) fn with_body(&mut self, body: HirBodyId, f: impl FnOnce(&mut Self)) {
        self.body_stack.push(body);
        let root_scope = self.body_mut(body).root_scope;
        self.scopes.push(ActiveScope {
            id: root_scope,
            locals: BTreeMap::new(),
        });
        f(self);
        if self
            .scopes
            .last()
            .is_some_and(|scope| scope.id == root_scope)
        {
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
        let mut body = HirBody::new(id, owner, HirSourceOrigin { source, span }, root_scope);
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
        let scope = self.current_scope();
        self.body_mut(body).blocks.insert(
            id,
            HirBlock {
                id,
                origin: HirSourceOrigin { source, span },
                scope,
                statements: Vec::new(),
            },
        );
        id
    }

    pub(super) fn next_stmt(&mut self, span: Span) -> HirStmtId {
        let id = HirStmtId::new(*self.next_stmt_id);
        *self.next_stmt_id = self.next_stmt_id.saturating_add(1);
        let body = self.current_body();
        let source = self.source;
        let scope = self.current_scope();
        self.body_mut(body).statements.insert(
            id,
            HirStmt {
                id,
                origin: HirSourceOrigin { source, span },
                scope,
                kind: HirStmtKind::Expr {
                    expression: None,
                    terminated: false,
                },
            },
        );
        if let Some(block) = self.block_stack.last().copied()
            && let Some(block) = self.body_mut(body).blocks.get_mut(&block)
        {
            block.statements.push(id);
        }
        id
    }

    pub(super) fn finish_stmt(&mut self, statement: HirStmtId, kind: HirStmtKind) {
        let body = self.current_body();
        self.body_mut(body)
            .statements
            .get_mut(&statement)
            .expect("reserved HIR statement should exist")
            .kind = kind;
    }

    pub(super) fn next_expr(&mut self, span: Span) -> HirExprId {
        let id = HirExprId::new(*self.next_expr_id);
        *self.next_expr_id = self.next_expr_id.saturating_add(1);
        let body = self.current_body();
        let source = self.source;
        let scope = self.current_scope();
        self.body_mut(body).expressions.insert(
            id,
            HirExpr {
                id,
                origin: HirSourceOrigin { source, span },
                scope,
                kind: HirExprKind::Literal(HirLiteral::Invalid {
                    source_text: String::new(),
                }),
            },
        );
        id
    }

    pub(super) fn finish_expr(&mut self, expression: HirExprId, kind: HirExprKind) {
        let body = self.current_body();
        self.body_mut(body)
            .expressions
            .get_mut(&expression)
            .expect("reserved HIR expression should exist")
            .kind = kind;
    }

    pub(super) fn missing_expr(&mut self, span: Span) -> HirExprId {
        let expression = self.next_expr(span);
        self.finish_expr(expression, HirExprKind::Missing);
        expression
    }

    pub(super) fn next_path(
        &mut self,
        owner: HirPathOwner,
        kind: HirPathKind,
        path: Vec<String>,
        origin_span: Span,
        segment_span: Span,
    ) -> HirPathId {
        let id = HirPathId::new(*self.next_path_id);
        *self.next_path_id = self.next_path_id.saturating_add(1);
        let body = self.current_body();
        let source = self.source;
        self.body_mut(body).paths.insert(
            id,
            HirPath {
                id,
                owner,
                kind,
                path,
                origin: HirSourceOrigin {
                    source,
                    span: origin_span,
                },
                segment_origin: HirSourceOrigin {
                    source,
                    span: segment_span,
                },
            },
        );
        id
    }

    pub(super) fn next_pattern(&mut self, span: Span) -> HirPatternId {
        let id = HirPatternId::new(*self.next_pattern_id);
        *self.next_pattern_id = self.next_pattern_id.saturating_add(1);
        let body = self.current_body();
        let source = self.source;
        let scope = self.current_scope();
        self.body_mut(body).patterns.insert(
            id,
            HirPattern {
                id,
                origin: HirSourceOrigin { source, span },
                scope,
                kind: HirPatternKind::Missing,
            },
        );
        id
    }

    pub(super) fn finish_pattern(&mut self, pattern: HirPatternId, kind: HirPatternKind) {
        let body = self.current_body();
        self.body_mut(body)
            .patterns
            .get_mut(&pattern)
            .expect("reserved HIR pattern should exist")
            .kind = kind;
    }

    pub(super) fn next_match_arm(
        &mut self,
        span: Span,
        scope: HirScopeId,
        pattern: Option<HirPatternId>,
        guard: Option<HirExprId>,
        arm_body: Option<HirMatchArmBody>,
    ) -> HirMatchArmId {
        let id = HirMatchArmId::new(*self.next_match_arm_id);
        *self.next_match_arm_id = self.next_match_arm_id.saturating_add(1);
        let body = self.current_body();
        let source = self.source;
        self.body_mut(body).match_arms.insert(
            id,
            HirMatchArm {
                id,
                origin: HirSourceOrigin { source, span },
                scope,
                pattern,
                guard,
                body: arm_body,
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
        let declaring_body = self.local_bodies.get(local).copied();
        let body_stack = self.body_stack.clone();
        for body in body_stack.into_iter().rev() {
            if declaring_body == Some(body) {
                break;
            }
            if self
                .bodies
                .get(&body)
                .is_some_and(|body| matches!(body.owner, HirBodyOwner::Lambda { .. }))
            {
                self.next_capture(body, *local, expression);
            }
        }
    }

    pub(super) fn record_self_use(
        &mut self,
        expression: HirExprId,
        resolution: &BindingResolution,
    ) {
        let BindingResolution::Local(local) = resolution else {
            return;
        };
        let body = self.current_body();
        if self.body_mut(body).self_binding == Some(*local) {
            self.body_mut(body).self_uses.push(expression);
        }
    }

    pub(super) fn record_unresolved_reference(
        &mut self,
        expression: HirExprId,
        name: String,
        span: Span,
    ) {
        let body = self.current_body();
        let source = self.source;
        self.body_mut(body)
            .unresolved_references
            .push(HirUnresolvedReference {
                expression,
                name,
                origin: HirSourceOrigin { source, span },
            });
    }
}
