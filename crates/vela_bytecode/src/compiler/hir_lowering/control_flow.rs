use super::*;

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn compile_hir_root_body(&mut self) -> CompileResult<bool> {
        self.compile_hir_body_root(self.body)
    }

    pub(in crate::compiler) fn compile_hir_body_root(
        &mut self,
        body: HirBodyId,
    ) -> CompileResult<bool> {
        let root = self
            .hir_bodies
            .iter()
            .find(|candidate| candidate.id == body)
            .map(|body| body.root)
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR body")))?;
        match root {
            HirBodyRoot::Block(block) => self.compile_hir_block(block),
            HirBodyRoot::Expr(expression) => {
                let value = self.compile_hir_expression(expression)?;
                self.emit(UnlinkedInstructionKind::Return { src: value });
                Ok(true)
            }
            HirBodyRoot::Empty => Ok(false),
        }
    }

    pub(in crate::compiler) fn compile_hir_value_body(
        mut self,
        body: HirBodyId,
    ) -> CompileResult<crate::UnlinkedCodeObject> {
        let root = self
            .hir_bodies
            .iter()
            .find(|candidate| candidate.id == body)
            .map(|body| body.root)
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR body")))?;
        match root {
            HirBodyRoot::Expr(expression) => {
                let value = self.compile_hir_expression(expression)?;
                self.emit(UnlinkedInstructionKind::Return { src: value });
            }
            HirBodyRoot::Block(block) => {
                let dst = self.alloc_register()?;
                if !self.compile_hir_block_value_to(block, dst)? {
                    self.emit(UnlinkedInstructionKind::Return { src: dst });
                }
            }
            HirBodyRoot::Empty => {
                let value = self.emit_constant(Constant::Unit)?;
                self.emit(UnlinkedInstructionKind::Return { src: value });
            }
        }
        self.code.register_count = self.next_register;
        Ok(self.code)
    }

    pub(in crate::compiler) fn compile_hir_block(
        &mut self,
        block: HirBlockId,
    ) -> CompileResult<bool> {
        let statements = self
            .hir_bodies
            .iter()
            .find_map(|body| body.blocks.get(&block))
            .map(|block| block.statements.clone())
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR block")))?;
        for statement in statements {
            if self.compile_hir_statement(statement)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(in crate::compiler) fn compile_hir_block_value_to(
        &mut self,
        block: HirBlockId,
        dst: Register,
    ) -> CompileResult<bool> {
        let statements = self
            .hir_bodies
            .iter()
            .find_map(|body| body.blocks.get(&block))
            .map(|block| block.statements.clone())
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR block")))?;
        let tail = statements.last().and_then(|statement| {
            self.hir_bodies
                .iter()
                .find_map(|body| body.statements.get(statement))
                .map(|statement| statement.kind.clone())
                .filter(|kind| {
                    matches!(
                        kind,
                        HirStmtKind::Expr {
                            expression: Some(_),
                            ..
                        } | HirStmtKind::If(_)
                            | HirStmtKind::Match(_)
                    )
                })
        });
        let prefix_len = statements.len().saturating_sub(usize::from(tail.is_some()));
        for statement in statements.into_iter().take(prefix_len) {
            if self.compile_hir_statement(statement)? {
                return Ok(true);
            }
        }
        if let Some(tail) = tail {
            match tail {
                HirStmtKind::Expr {
                    expression: Some(expression),
                    ..
                } => {
                    let value = self.compile_hir_expression(expression)?;
                    self.emit(UnlinkedInstructionKind::Move { dst, src: value });
                }
                HirStmtKind::If(value) => {
                    self.compile_hir_if_value_to(&value, dst)?;
                }
                HirStmtKind::Match(value) => {
                    self.compile_hir_match(&value, Some(dst))?;
                }
                _ => unreachable!("HIR block value tail was filtered above"),
            }
        } else {
            self.emit_constant_to(dst, Constant::Unit);
        }
        Ok(false)
    }

    pub(in crate::compiler) fn compile_hir_statement(
        &mut self,
        statement: HirStmtId,
    ) -> CompileResult<bool> {
        let (span, kind) = self
            .hir_bodies
            .iter()
            .find_map(|body| body.statements.get(&statement))
            .map(|statement| (statement.origin.span, statement.kind.clone()))
            .ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR statement"))
            })?;
        match kind {
            HirStmtKind::Let {
                pattern,
                type_hint,
                initializer,
            } => {
                let value = match initializer {
                    Some(initializer) => {
                        let binding_name = pattern
                            .and_then(|pattern| {
                                self.hir_bodies
                                    .iter()
                                    .find_map(|body| body.patterns.get(&pattern))
                                    .and_then(|pattern| pattern.local())
                            })
                            .and_then(|local| self.bindings.local(local))
                            .map(|binding| binding.name.clone())
                            .unwrap_or_else(|| "local".to_owned());
                        self.compile_hir_let_initializer(
                            initializer,
                            type_hint.as_ref(),
                            &binding_name,
                            span,
                        )?
                    }
                    None => self.emit_constant(Constant::Unit)?,
                };
                if let Some(pattern) = pattern {
                    self.bind_hir_let_pattern(value, pattern, span, initializer)?;
                }
                Ok(false)
            }
            HirStmtKind::Return { value } => {
                let src = match value {
                    Some(value) => self.compile_hir_expression(value)?,
                    None => self.emit_constant(Constant::Unit)?,
                };
                self.emit_spanned(UnlinkedInstructionKind::Return { src }, span);
                Ok(true)
            }
            HirStmtKind::Break => self.compile_break(),
            HirStmtKind::Continue => self.compile_continue(),
            HirStmtKind::Block(block) => self.compile_hir_block(block),
            HirStmtKind::Expr {
                expression: Some(expression),
                ..
            } => {
                self.compile_hir_expression(expression)?;
                Ok(false)
            }
            HirStmtKind::Expr {
                expression: None, ..
            } => Ok(false),
            HirStmtKind::If(value) => self.compile_hir_if_statement(&value),
            HirStmtKind::For {
                patterns,
                iterable: Some(iterable),
                body: Some(body),
            } => self.compile_hir_for(span, &patterns, iterable, body),
            HirStmtKind::Match(value) => self.compile_hir_match(&value, None),
            HirStmtKind::For { .. } => Err(hir_unsupported("statement", span)),
        }
    }

    pub(in crate::compiler) fn compile_hir_for(
        &mut self,
        span: Span,
        patterns: &[HirPatternId],
        iterable: HirExprId,
        body: HirBlockId,
    ) -> CompileResult<bool> {
        let range = match self.hir_expression_record(iterable)?.1 {
            HirExprKind::Binary {
                op: Some(HirBinaryOp::Range),
                lhs: Some(start),
                rhs: Some(end),
            } => Some((start, end, false)),
            HirExprKind::Binary {
                op: Some(HirBinaryOp::RangeInclusive),
                lhs: Some(start),
                rhs: Some(end),
            } => Some((start, end, true)),
            _ => None,
        };
        let item_facts = if range.is_some() {
            PatternBindingFacts::value(Some(RuntimeTypeFact::primitive(
                vela_common::PrimitiveTag::I64,
            )))
        } else {
            let item_shape = self
                .value_shape_for_hir_expression(iterable)
                .and_then(|shape| match shape {
                    crate::compiler::record_shapes::ValueShape::Array(element)
                    | crate::compiler::record_shapes::ValueShape::Set(element) => Some(*element),
                    crate::compiler::record_shapes::ValueShape::Map { key, value } => Some(
                        crate::compiler::record_shapes::ValueShape::map_entry(*key, *value),
                    ),
                    _ => None,
                });
            PatternBindingFacts::value_shape(item_shape)
        };
        let loop_iterable = if let Some((start, end, inclusive)) = range {
            let cursor = self.compile_hir_expression(start)?;
            let end = self.compile_hir_expression(end)?;
            let done = self.alloc_register()?;
            self.emit_bool_constant_to(done, false);
            LoopIterable::Range {
                cursor,
                end,
                done,
                inclusive,
            }
        } else {
            let iterable_register = self.compile_hir_expression(iterable)?;
            let iterator = self.alloc_register()?;
            self.emit_spanned(
                UnlinkedInstructionKind::IterInit {
                    dst: iterator,
                    iterable: iterable_register,
                },
                self.expression_span(iterable).unwrap_or(span),
            );
            LoopIterable::Generic { iterator }
        };
        let item_register = self.alloc_register()?;
        let has_index = patterns.len() > 1;
        let loop_index = if has_index {
            let counter = self.alloc_register()?;
            self.emit_constant_to(counter, Constant::Scalar(vela_common::ScalarValue::I64(0)));
            Some((
                counter,
                self.emit_constant(Constant::Scalar(vela_common::ScalarValue::I64(1)))?,
                self.alloc_register()?,
            ))
        } else {
            None
        };

        let previous_locals = self.locals.clone();
        let previous_hir_locals = self.hir_locals.clone();
        let previous_script_types = self.script_types.clone();
        let previous_value_types = self.value_types.clone();
        let previous_value_shapes = self.value_shapes.clone();
        let loop_start = self.current_offset();
        let done_jump = match loop_iterable {
            LoopIterable::Generic { iterator } => self.emit_iter_next(iterator, item_register),
            LoopIterable::Range {
                cursor,
                end,
                done,
                inclusive,
            } => self.emit_range_next(cursor, end, done, inclusive, item_register),
        };
        let mut mismatch_jumps = Vec::new();
        if let Some((counter, one, index_register)) = loop_index {
            self.emit(UnlinkedInstructionKind::Move {
                dst: index_register,
                src: counter,
            });
            self.emit(UnlinkedInstructionKind::Add {
                dst: counter,
                lhs: counter,
                rhs: one,
            });
            if let Some(index_pattern) = patterns.first() {
                mismatch_jumps
                    .extend(self.compile_hir_match_pattern(index_register, *index_pattern)?);
                self.bind_hir_pattern_locals(
                    index_register,
                    *index_pattern,
                    span,
                    PatternBindingFacts::value(Some(RuntimeTypeFact::primitive(
                        vela_common::PrimitiveTag::I64,
                    ))),
                    LocalBindingKind::For,
                )?;
            }
        }
        if let Some(value_pattern) = patterns.last() {
            mismatch_jumps.extend(self.compile_hir_match_pattern(item_register, *value_pattern)?);
            self.bind_hir_pattern_locals(
                item_register,
                *value_pattern,
                span,
                item_facts,
                LocalBindingKind::For,
            )?;
        }

        self.loop_stack.push(LoopContext::new(loop_start));
        let body_returned = self.compile_hir_block(body)?;
        let loop_context = self.loop_stack.pop().expect("loop context was pushed");
        if !body_returned {
            self.emit(UnlinkedInstructionKind::Jump {
                target: crate::InstructionOffset(loop_start),
            });
        }
        let loop_end = self.current_offset();
        self.patch_jump(done_jump, loop_end)?;
        for jump in mismatch_jumps {
            self.patch_jump(jump, loop_start)?;
        }
        for jump in loop_context.break_jumps() {
            self.patch_jump(*jump, loop_end)?;
        }
        for jump in loop_context.continue_jumps() {
            self.patch_jump(*jump, loop_context.continue_target())?;
        }
        self.locals = previous_locals;
        self.hir_locals = previous_hir_locals;
        self.script_types = previous_script_types;
        self.value_types = previous_value_types;
        self.value_shapes = previous_value_shapes;
        Ok(false)
    }

    pub(in crate::compiler) fn bind_hir_let_pattern(
        &mut self,
        value: Register,
        pattern: HirPatternId,
        span: Span,
        initializer: Option<HirExprId>,
    ) -> CompileResult<()> {
        let local = self
            .hir_bodies
            .iter()
            .find_map(|body| body.patterns.get(&pattern))
            .and_then(|pattern| pattern.local());
        if let Some(local) = local
            && let Some(binding) = self.bindings.local(local).cloned()
        {
            let aliases_existing = initializer.is_some_and(|initializer| {
                self.hir_expression_aliases_existing_register(initializer)
            });
            let local_value = if aliases_existing {
                let copy = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::Move {
                    dst: copy,
                    src: value,
                });
                copy
            } else {
                value
            };
            self.locals.insert(binding.name.clone(), local_value);
            self.hir_locals.insert(local, local_value);
            let inferred_script =
                initializer.and_then(|expression| self.script_fact_for_hir_expression(expression));
            let hinted_script = binding.type_hint.as_ref().and_then(|hint| {
                crate::compiler::script_types::type_hint_script_type(
                    hint,
                    self.facts.known_type_names().iter(),
                )
                .map(crate::compiler::script_types::ScriptTypeFact::new)
            });
            let script_fact = match (hinted_script, inferred_script) {
                (Some(hinted), Some(inferred)) if hinted.type_name == inferred.type_name => {
                    Some(crate::compiler::script_types::ScriptTypeFact {
                        type_name: hinted.type_name,
                        enum_variant: inferred.enum_variant,
                    })
                }
                (Some(hinted), _) => Some(hinted),
                (None, inferred) => inferred,
            };
            self.script_types
                .set_local_fact(local, binding.name.clone(), script_fact.clone());
            let value_fact = binding
                .type_hint
                .as_ref()
                .and_then(crate::compiler::value_types::type_hint_value_type)
                .or_else(|| initializer.and_then(|expression| self.hir_value_type(expression)));
            self.value_types
                .set_local(local, binding.name.clone(), value_fact);
            let value_shape = script_fact
                .as_ref()
                .and_then(|fact| self.record_shape_for_type(&fact.type_name))
                .map(crate::compiler::record_shapes::ValueShape::Record)
                .or_else(|| {
                    initializer
                        .and_then(|expression| self.value_shape_for_hir_expression(expression))
                });
            self.value_shapes
                .set_local(local, binding.name.clone(), value_shape);
            self.record_frame_slot(
                binding.name,
                local_value,
                frame_slot_kind(LocalBindingKind::Let),
                Some(local),
                Some(span),
            );
            return Ok(());
        }
        self.bind_hir_pattern_locals(
            value,
            pattern,
            span,
            PatternBindingFacts::default(),
            LocalBindingKind::Let,
        )
    }

    pub(in crate::compiler) fn compile_hir_if_statement(
        &mut self,
        value: &HirIf,
    ) -> CompileResult<bool> {
        let condition = value.condition.ok_or_else(|| {
            CompileError::new(CompileErrorKind::UnsupportedSyntax("if condition"))
        })?;
        let else_jump = self.compile_hir_jump_if_false(condition)?;
        let then_returned = match value.then_block {
            Some(block) => self.compile_hir_block(block)?,
            None => false,
        };
        let end_jump = self.emit_jump();
        self.patch_jump(else_jump, self.current_offset())?;
        let else_returned = match value.else_branch.as_ref() {
            Some(HirElseBranch::If(value)) => self.compile_hir_if_statement(value)?,
            Some(HirElseBranch::Block(block)) => self.compile_hir_block(*block)?,
            None => false,
        };
        self.patch_jump(end_jump, self.current_offset())?;
        Ok(then_returned && else_returned)
    }

    pub(in crate::compiler) fn compile_hir_match(
        &mut self,
        value: &HirMatch,
        dst: Option<Register>,
    ) -> CompileResult<bool> {
        let scrutinee_expression = value.scrutinee.ok_or_else(|| {
            CompileError::new(CompileErrorKind::UnsupportedSyntax("match scrutinee"))
        })?;
        let scrutinee = self.compile_hir_expression(scrutinee_expression)?;
        let scrutinee_facts = PatternBindingFacts::value_shape(
            self.value_shape_for_hir_expression(scrutinee_expression),
        )
        .with_script(self.script_fact_for_hir_expression(scrutinee_expression));
        let mut end_jumps = Vec::new();
        let mut all_arms_return = !value.arms.is_empty();
        let mut has_catch_all = false;

        for arm_id in &value.arms {
            let arm = self
                .hir_bodies
                .iter()
                .find_map(|body| body.match_arms.get(arm_id))
                .cloned()
                .ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR match arm"))
                })?;
            let pattern = arm
                .pattern
                .ok_or_else(|| hir_unsupported("match pattern", arm.origin.span))?;
            let mut next_arm_jumps = self.compile_hir_match_pattern(scrutinee, pattern)?;
            let previous_locals = self.locals.clone();
            let previous_hir_locals = self.hir_locals.clone();
            let previous_script_types = self.script_types.clone();
            let previous_value_types = self.value_types.clone();
            let previous_value_shapes = self.value_shapes.clone();
            let arm_scrutinee = if matches!(
                self.hir_bodies
                    .iter()
                    .find_map(|body| body.patterns.get(&pattern))
                    .map(|pattern| &pattern.kind),
                Some(HirPatternKind::Binding { .. })
            ) {
                let copy = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::Move {
                    dst: copy,
                    src: scrutinee,
                });
                copy
            } else {
                scrutinee
            };
            self.bind_hir_pattern_locals(
                arm_scrutinee,
                pattern,
                arm.origin.span,
                scrutinee_facts.clone(),
                LocalBindingKind::Pattern,
            )?;
            if let Some(guard) = arm.guard {
                let condition = self.compile_hir_expression(guard)?;
                next_arm_jumps.push(self.emit_jump_if_false(condition));
            }
            let arm_returned = match arm.body {
                Some(HirMatchArmBody::Expr(expression)) => {
                    let result = self.compile_hir_expression(expression)?;
                    if let Some(dst) = dst {
                        self.emit(UnlinkedInstructionKind::Move { dst, src: result });
                    }
                    false
                }
                Some(HirMatchArmBody::Block(block)) => match dst {
                    Some(dst) => self.compile_hir_block_value_to(block, dst)?,
                    None => self.compile_hir_block(block)?,
                },
                None => return Err(hir_unsupported("match arm body", arm.origin.span)),
            };
            self.locals = previous_locals;
            self.hir_locals = previous_hir_locals;
            self.script_types = previous_script_types;
            self.value_types = previous_value_types;
            self.value_shapes = previous_value_shapes;
            all_arms_return &= arm_returned;
            if !arm_returned {
                end_jumps.push(self.emit_jump());
            }
            if next_arm_jumps.is_empty() {
                has_catch_all = true;
                break;
            }
            for jump in next_arm_jumps {
                self.patch_jump(jump, self.current_offset())?;
            }
        }
        if let Some(dst) = dst
            && !has_catch_all
        {
            self.emit_constant_to(dst, Constant::Unit);
        }
        for jump in end_jumps {
            self.patch_jump(jump, self.current_offset())?;
        }
        Ok(all_arms_return)
    }

    fn hir_expression_aliases_existing_register(&self, mut expression: HirExprId) -> bool {
        loop {
            match self.hir_expression_record(expression).map(|(_, kind)| kind) {
                Ok(HirExprKind::Path(_)) => return true,
                Ok(HirExprKind::Paren {
                    expression: Some(inner),
                }) => expression = inner,
                _ => return false,
            }
        }
    }

    pub(in crate::compiler) fn compile_hir_match_pattern(
        &mut self,
        scrutinee: Register,
        pattern: HirPatternId,
    ) -> CompileResult<Vec<usize>> {
        let (span, kind) = self
            .hir_bodies
            .iter()
            .find_map(|body| body.patterns.get(&pattern))
            .map(|pattern| (pattern.origin.span, pattern.kind.clone()))
            .ok_or_else(|| CompileError::new(CompileErrorKind::UnsupportedSyntax("HIR pattern")))?;
        match kind {
            HirPatternKind::Wildcard | HirPatternKind::Binding { .. } => Ok(Vec::new()),
            HirPatternKind::Literal(Some(literal)) => {
                let pattern = self.compile_hir_literal(span, &literal)?;
                let condition = self.alloc_register()?;
                self.emit(UnlinkedInstructionKind::Equal {
                    dst: condition,
                    lhs: scrutinee,
                    rhs: pattern,
                });
                Ok(vec![self.emit_jump_if_false(condition)])
            }
            HirPatternKind::Path { path: Some(path) } => {
                let path = self.hir_pattern_path_by_id(path, span)?;
                self.compile_variant_tag_pattern(scrutinee, &path)
            }
            HirPatternKind::TupleVariant { path, fields } => {
                let path = path
                    .map(|path| self.hir_pattern_path_by_id(path, span))
                    .transpose()?;
                let mut jumps = if let Some(path) = path.as_ref() {
                    self.compile_variant_tag_pattern(scrutinee, path)?
                } else {
                    let condition = self.alloc_register()?;
                    self.emit(UnlinkedInstructionKind::TupleArityEqual {
                        dst: condition,
                        value: scrutinee,
                        arity: fields.len(),
                    });
                    vec![self.emit_jump_if_false(condition)]
                };
                for (index, field) in fields.into_iter().enumerate() {
                    if !self.hir_pattern_needs_check(field) {
                        continue;
                    }
                    let value = if let Some(path) = path.as_ref() {
                        self.emit_enum_pattern_field_read(scrutinee, path, index.to_string())?
                    } else {
                        self.emit_tuple_pattern_field_read(scrutinee, index)?
                    };
                    jumps.extend(self.compile_hir_match_pattern(value, field)?);
                }
                Ok(jumps)
            }
            HirPatternKind::RecordVariant {
                path: Some(path),
                fields,
            } => {
                let path = self.hir_pattern_path_by_id(path, span)?;
                let mut jumps = self.compile_variant_tag_pattern(scrutinee, &path)?;
                for field in fields {
                    let Some(pattern) = field.pattern else {
                        continue;
                    };
                    if !self.hir_pattern_needs_check(pattern) {
                        continue;
                    }
                    let value = self.emit_enum_pattern_field_read(scrutinee, &path, field.name)?;
                    jumps.extend(self.compile_hir_match_pattern(value, pattern)?);
                }
                Ok(jumps)
            }
            HirPatternKind::Literal(None)
            | HirPatternKind::Path { path: None }
            | HirPatternKind::RecordVariant { path: None, .. }
            | HirPatternKind::Missing => Err(hir_unsupported("match pattern", span)),
        }
    }

    pub(in crate::compiler) fn hir_pattern_needs_check(&self, pattern: HirPatternId) -> bool {
        self.hir_bodies
            .iter()
            .find_map(|body| body.patterns.get(&pattern))
            .is_some_and(|pattern| {
                matches!(
                    pattern.kind,
                    HirPatternKind::Literal(Some(_))
                        | HirPatternKind::Path { path: Some(_) }
                        | HirPatternKind::TupleVariant { .. }
                        | HirPatternKind::RecordVariant { .. }
                )
            })
    }

    pub(in crate::compiler) fn hir_pattern_path_by_id(
        &self,
        path: vela_hir::ids::HirPathId,
        span: Span,
    ) -> CompileResult<Vec<String>> {
        self.hir_bodies
            .iter()
            .find_map(|body| body.paths.get(&path))
            .map(|path| path.path.clone())
            .ok_or_else(|| hir_unsupported("pattern path", span))
    }
}
