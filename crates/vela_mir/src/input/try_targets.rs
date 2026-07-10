use std::collections::btree_map::Entry;

use vela_def::{FieldId, FunctionId, TypeId, VariantId};
use vela_hir::ids::HirExprId;

use crate::MirSourceOrigin;

use super::{
    CompileTargetKind, CompileTargetSnapshot, CompileTargetSnapshotBuilder, MirBuildError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompileTryFamily {
    Option,
    Result,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileTryLayoutTarget {
    pub family: CompileTryFamily,
    pub type_id: TypeId,
    pub continue_variant: VariantId,
    pub break_variant: VariantId,
    pub continue_payload: FieldId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileTryTarget {
    Expected(CompileTryLayoutTarget),
    Dynamic {
        option: CompileTryLayoutTarget,
        result: CompileTryLayoutTarget,
    },
}

impl CompileTargetSnapshot {
    pub(super) fn try_target(
        &self,
        function: FunctionId,
        expression: HirExprId,
    ) -> Option<&CompileTryTarget> {
        self.try_targets.get(&(function, expression))
    }
}

impl CompileTargetSnapshotBuilder {
    pub fn insert_try_target(
        &mut self,
        function: FunctionId,
        expression: HirExprId,
        target: CompileTryTarget,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        match self.snapshot.try_targets.entry((function, expression)) {
            Entry::Vacant(entry) => {
                entry.insert(target);
                self.snapshot
                    .origins
                    .try_targets
                    .insert((function, expression), origin);
                Ok(())
            }
            Entry::Occupied(_) => Err(MirBuildError::DuplicateCompileTarget {
                function,
                kind: CompileTargetKind::Try,
                expression,
                origin,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use vela_common::{SourceId, Span};
    use vela_def::{FieldId, FunctionId, TypeId, VariantId};
    use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId};

    use crate::{
        CompileFieldAccess, CompileFieldDescriptor, CompileFunctionAccess, CompileFunctionClass,
        CompileFunctionDescriptor, CompileFunctionTargets, CompilePositionalPolicy,
        CompileSignature, CompileTargetKind, CompileTargetSnapshot, CompileTargetSnapshotBuilder,
        CompileTypeClass, CompileTypeDescriptor, CompileVariantDescriptor, MirBuildError,
        MirEffect, MirSourceOrigin,
    };

    use super::{CompileTryFamily, CompileTryLayoutTarget, CompileTryTarget};

    fn origin(seed: u32) -> MirSourceOrigin {
        MirSourceOrigin::declaration(
            HirDeclId::new(seed),
            Span::new(SourceId::new(51), seed, seed + 1),
        )
    }

    fn insert_root(
        builder: &mut CompileTargetSnapshotBuilder,
        function: FunctionId,
        declaration: HirDeclId,
        body: HirBodyId,
        root_origin: MirSourceOrigin,
    ) {
        builder
            .insert_script_function(
                declaration,
                body,
                CompileFunctionDescriptor {
                    id: function,
                    class: CompileFunctionClass::Script,
                    canonical_symbol: format!("test::try_root_{}", function.get()),
                    debug_name: format!("try_root_{}", function.get()),
                    signature: CompileSignature {
                        parameters: Vec::new(),
                        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                        return_contract: None,
                        effect: MirEffect::PURE,
                    },
                    access: CompileFunctionAccess::script(false),
                },
                root_origin,
            )
            .expect("root fixture should be unique");
    }

    fn insert_layout(
        builder: &mut CompileTargetSnapshotBuilder,
        family: CompileTryFamily,
        seed: u128,
        schema_origin: MirSourceOrigin,
    ) -> CompileTryLayoutTarget {
        let type_id = TypeId::new(seed);
        let continue_variant = VariantId::new(seed + 1);
        let break_variant = VariantId::new(seed + 2);
        let continue_payload = FieldId::new(seed + 3);
        builder
            .insert_type_descriptor(
                CompileTypeDescriptor {
                    id: type_id,
                    canonical_name: format!("test::{family:?}_{seed}"),
                    class: CompileTypeClass::Standard,
                    shape: None,
                    fields: Vec::new(),
                    variants: vec![continue_variant, break_variant],
                },
                schema_origin,
            )
            .expect("try owner fixture should be unique");
        builder
            .insert_variant_descriptor(
                CompileVariantDescriptor {
                    id: continue_variant,
                    owner: type_id,
                    name: "Continue".to_owned(),
                    fields: vec![continue_payload],
                    declaration_order: 0,
                },
                schema_origin,
            )
            .expect("continue variant fixture should be unique");
        builder
            .insert_variant_descriptor(
                CompileVariantDescriptor {
                    id: break_variant,
                    owner: type_id,
                    name: "Break".to_owned(),
                    fields: Vec::new(),
                    declaration_order: 1,
                },
                schema_origin,
            )
            .expect("break variant fixture should be unique");
        builder
            .insert_field_descriptor(
                CompileFieldDescriptor {
                    id: continue_payload,
                    owner: type_id,
                    variant: Some(continue_variant),
                    name: "value".to_owned(),
                    contract: None,
                    declaration_order: 0,
                    access: CompileFieldAccess::script(),
                    host_runtime: None,
                },
                schema_origin,
            )
            .expect("continue payload fixture should be unique");
        CompileTryLayoutTarget {
            family,
            type_id,
            continue_variant,
            break_variant,
            continue_payload,
        }
    }

    fn assert_input_error(
        error: MirBuildError,
        expected_origin: MirSourceOrigin,
        expected_text: &str,
    ) {
        assert_eq!(error.origin(), Some(expected_origin));
        assert!(
            error.to_string().contains(expected_text),
            "expected {error:?} to contain {expected_text:?}"
        );
    }

    #[test]
    fn try_targets_are_validated_and_looked_up_through_their_executable_root() {
        let first_function = FunctionId::new(700);
        let second_function = FunctionId::new(701);
        let expression = HirExprId::new(702);
        let root_origin = origin(703);
        let target_origin = origin(704);
        let mut builder = CompileTargetSnapshot::builder();
        insert_root(
            &mut builder,
            first_function,
            HirDeclId::new(705),
            HirBodyId::new(706),
            root_origin,
        );
        insert_root(
            &mut builder,
            second_function,
            HirDeclId::new(707),
            HirBodyId::new(708),
            root_origin,
        );
        let option = insert_layout(&mut builder, CompileTryFamily::Option, 710, root_origin);
        let result = insert_layout(&mut builder, CompileTryFamily::Result, 720, root_origin);
        let expected = CompileTryTarget::Expected(option);
        let dynamic = CompileTryTarget::Dynamic { option, result };
        builder
            .insert_try_target(first_function, expression, expected, target_origin)
            .expect("the first root should own its try target");
        builder
            .insert_try_target(second_function, expression, dynamic, target_origin)
            .expect("the second root may reuse the expression identity");

        let snapshot = builder.build().expect("closed try layouts should validate");
        let first = CompileFunctionTargets::new(
            &snapshot,
            snapshot
                .function(first_function)
                .expect("first executable root"),
        );
        let second = CompileFunctionTargets::new(
            &snapshot,
            snapshot
                .function(second_function)
                .expect("second executable root"),
        );
        assert_eq!(first.try_target(expression), Some(&expected));
        assert_eq!(second.try_target(expression), Some(&dynamic));
    }

    #[test]
    fn duplicate_try_target_reports_the_try_target_kind() {
        let function = FunctionId::new(730);
        let expression = HirExprId::new(731);
        let first_origin = origin(732);
        let duplicate_origin = origin(733);
        let target = CompileTryTarget::Expected(CompileTryLayoutTarget {
            family: CompileTryFamily::Option,
            type_id: TypeId::new(734),
            continue_variant: VariantId::new(735),
            break_variant: VariantId::new(736),
            continue_payload: FieldId::new(737),
        });
        let mut builder = CompileTargetSnapshot::builder();
        builder
            .insert_try_target(function, expression, target, first_origin)
            .expect("first try target should be inserted");
        let error = builder
            .insert_try_target(function, expression, target, duplicate_origin)
            .expect_err("duplicate try targets must be rejected");
        assert!(matches!(
            error,
            MirBuildError::DuplicateCompileTarget {
                function: duplicate_function,
                kind: CompileTargetKind::Try,
                expression: duplicate_expression,
                origin,
            } if duplicate_function == function
                && duplicate_expression == expression
                && origin == duplicate_origin
        ));
    }

    #[test]
    fn try_target_requires_an_executable_root_and_retains_its_origin() {
        let function = FunctionId::new(740);
        let expression = HirExprId::new(741);
        let schema_origin = origin(742);
        let target_origin = origin(743);
        let mut builder = CompileTargetSnapshot::builder();
        let option = insert_layout(&mut builder, CompileTryFamily::Option, 744, schema_origin);
        builder
            .insert_try_target(
                function,
                expression,
                CompileTryTarget::Expected(option),
                target_origin,
            )
            .expect("try target fixture should be unique");

        assert_input_error(
            builder.build().expect_err("missing root must fail closure"),
            target_origin,
            "missing executable root",
        );
    }

    #[test]
    fn try_target_rejects_cross_type_variant_edges() {
        let function = FunctionId::new(760);
        let expression = HirExprId::new(761);
        let schema_origin = origin(762);
        let target_origin = origin(763);
        let mut builder = CompileTargetSnapshot::builder();
        insert_root(
            &mut builder,
            function,
            HirDeclId::new(764),
            HirBodyId::new(765),
            schema_origin,
        );
        let option = insert_layout(&mut builder, CompileTryFamily::Option, 770, schema_origin);
        let result = insert_layout(&mut builder, CompileTryFamily::Result, 780, schema_origin);
        builder
            .insert_try_target(
                function,
                expression,
                CompileTryTarget::Expected(CompileTryLayoutTarget {
                    break_variant: result.break_variant,
                    ..option
                }),
                target_origin,
            )
            .expect("try target fixture should be unique");

        assert_input_error(
            builder
                .build()
                .expect_err("cross-type variant edge must fail closure"),
            target_origin,
            "type-to-variant ownership",
        );
    }

    #[test]
    fn try_target_rejects_cross_variant_continue_payload_edges() {
        let function = FunctionId::new(790);
        let expression = HirExprId::new(791);
        let schema_origin = origin(792);
        let target_origin = origin(793);
        let mut builder = CompileTargetSnapshot::builder();
        insert_root(
            &mut builder,
            function,
            HirDeclId::new(794),
            HirBodyId::new(795),
            schema_origin,
        );
        let option = insert_layout(&mut builder, CompileTryFamily::Option, 800, schema_origin);
        let result = insert_layout(&mut builder, CompileTryFamily::Result, 810, schema_origin);
        builder
            .insert_try_target(
                function,
                expression,
                CompileTryTarget::Expected(CompileTryLayoutTarget {
                    continue_payload: result.continue_payload,
                    ..option
                }),
                target_origin,
            )
            .expect("try target fixture should be unique");

        assert_input_error(
            builder
                .build()
                .expect_err("cross-variant payload edge must fail closure"),
            target_origin,
            "continue-payload ownership",
        );
    }

    #[test]
    fn dynamic_try_target_requires_option_and_result_family_slots() {
        let function = FunctionId::new(820);
        let expression = HirExprId::new(821);
        let schema_origin = origin(822);
        let target_origin = origin(823);
        let mut builder = CompileTargetSnapshot::builder();
        insert_root(
            &mut builder,
            function,
            HirDeclId::new(824),
            HirBodyId::new(825),
            schema_origin,
        );
        let option = insert_layout(&mut builder, CompileTryFamily::Option, 830, schema_origin);
        let result = insert_layout(&mut builder, CompileTryFamily::Result, 840, schema_origin);
        builder
            .insert_try_target(
                function,
                expression,
                CompileTryTarget::Dynamic {
                    option: result,
                    result: option,
                },
                target_origin,
            )
            .expect("try target fixture should be unique");

        assert_input_error(
            builder
                .build()
                .expect_err("swapped dynamic family slots must fail closure"),
            target_origin,
            "Option and Result slots",
        );
    }
}
