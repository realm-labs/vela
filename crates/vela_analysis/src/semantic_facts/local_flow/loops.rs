use super::{LocalEnvironment, LocalFlow, LocalValue, ScriptTypeOrigins, join_environments};
use crate::semantic_facts::iterable_item_fact;
use crate::type_fact::TypeFact;
use vela_hir::binding::BindingResolution;
use vela_hir::body::HirExprKind;
use vela_hir::ids::{HirBlockId, HirExprId, HirPatternId};

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(super) struct LoopExits {
    pub(super) breaks: Vec<LocalEnvironment>,
    pub(super) continues: Vec<LocalEnvironment>,
}

impl LocalFlow<'_> {
    pub(super) fn visit_loop(
        &mut self,
        patterns: &[HirPatternId],
        iterable: Option<HirExprId>,
        body: Option<HirBlockId>,
        environment: &mut LocalEnvironment,
    ) -> bool {
        // Input exits belong to the enclosing loop, before this scope exists.
        if !self.visit_optional(iterable, environment) {
            return false;
        }
        let entry = environment.clone();
        let item = iterable
            .map(|id| iterable_item_fact(&self.fact(id, environment)))
            .unwrap_or(TypeFact::Unknown);
        let mut header = entry.clone();
        loop {
            // A body-wide budget bounds nested fixed-point work. Exhaustion
            // erases writable input refinements, then performs one final walk.
            let exhausted = self.loop_passes_left == 0;
            if exhausted {
                self.widen_loop_inputs(body, &mut header);
            } else {
                self.loop_passes_left -= 1;
            }
            let mut iteration = header.clone();
            for (index, pattern) in patterns.iter().enumerate() {
                let fact = if patterns.len() == 2 && index == 0 {
                    TypeFact::I64
                } else {
                    item.clone()
                };
                self.bind_pattern(*pattern, &fact, &Default::default(), &mut iteration);
            }
            self.loop_exits.push(LoopExits::default());
            let normal = body.is_none_or(|id| self.visit_block(id, &mut iteration));
            let exits = self.loop_exits.pop().expect("loop exit scope");
            let next = join_environments(
                [&header, &entry]
                    .into_iter()
                    .chain(normal.then_some(&iteration))
                    .chain(exits.continues.iter()),
                self.base,
            );
            if exhausted || next == header {
                *environment = join_environments(
                    [&header, &entry]
                        .into_iter()
                        .chain(normal.then_some(&iteration))
                        .chain(exits.continues.iter())
                        .chain(exits.breaks.iter()),
                    self.base,
                );
                return true;
            }
            header = next;
        }
    }

    fn widen_loop_inputs(&self, block: Option<HirBlockId>, environment: &mut LocalEnvironment) {
        let Some(block) = block.and_then(|id| self.body.blocks.get(&id)) else {
            return;
        };
        let span = block.origin.span;
        for expression in self.body.expressions.values() {
            if expression.origin.span.start < span.start || expression.origin.span.end > span.end {
                continue;
            }
            let HirExprKind::Assign {
                target: Some(target),
                ..
            } = expression.kind
            else {
                continue;
            };
            let Some(BindingResolution::Local(local)) = self.base.resolution(target) else {
                continue;
            };
            environment.insert(
                *local,
                LocalValue::new(
                    self.base
                        .base_local(*local)
                        .cloned()
                        .unwrap_or(TypeFact::Unknown),
                    self.base
                        .base_local_script_type(*local)
                        .cloned()
                        .map(ScriptTypeOrigins::known)
                        .unwrap_or_default(),
                ),
            );
        }
    }
}
