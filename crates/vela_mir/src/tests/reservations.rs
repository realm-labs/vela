use vela_common::{SourceId, Span};
use vela_def::{FunctionId, MethodId, TypeId};
use vela_hir::ids::{HirBodyId, HirExprId, HirNodeId};

use crate::*;

fn origin(body: HirBodyId, source: u32) -> MirSourceOrigin {
    MirSourceOrigin::body(body, Span::new(SourceId::new(source), 1, 8))
}

fn function(body: HirBodyId, owner: MirFunctionOwner, origin: MirSourceOrigin) -> MirFunction {
    MirFunction::new(body, owner, format!("body_{}", body.get()), None, origin)
}

fn method_target(function: u128, method: u128, owner: u128, node: u32) -> MethodExecutableTarget {
    MethodExecutableTarget {
        function: FunctionId::new(function),
        method: MethodId::new(method),
        owner: TypeId::new(owner),
        node: HirNodeId::new(node),
    }
}

#[test]
fn reservations_allow_parent_and_child_lambda_bodies_to_cross_reference() {
    let root_body = HirBodyId::new(700);
    let child_body = HirBodyId::new(701);
    let root_origin = origin(root_body, 70);
    let child_origin = origin(child_body, 71);
    let root_function = FunctionId::new(702);
    let root_owner = MirFunctionOwner::Function(root_function);
    let mut program = MirProgram::new(MirTargetTable::default());

    let root = program
        .reserve_function(root_body, root_owner.clone(), root_origin)
        .expect("root reservation should own the first generation-local ID");
    assert_eq!(program.function_by_id(root_function), Some(root));
    assert_eq!(program.functions_for_body(root_body), [root]);
    assert!(program.function(root).is_none());
    assert!(program.has_undefined_reservations());
    assert!(program.dump().contains("fn f0 reserved body h700"));

    let child_owner = MirFunctionOwner::Lambda {
        parent: root,
        expression: HirExprId::new(703),
    };
    let child = program
        .reserve_function(child_body, child_owner.clone(), child_origin)
        .expect("a lambda may name its reserved parent");
    assert_eq!(program.functions_for_body(child_body), [child]);
    assert_eq!(program.len(), 2);
    assert_eq!(program.defined_len(), 0);
    assert_eq!(
        program
            .undefined_reservations()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        [root, child]
    );
    assert_eq!(
        program.reservation(child).map(|value| value.owner()),
        Some(&child_owner)
    );

    let mut root_definition = function(root_body, root_owner, root_origin);
    let closure = root_definition.add_temp(MirValueType::Callable, root_origin);
    let closure_safepoint = root_definition.add_safepoint(MirSafepoint::new(root_origin));
    root_definition
        .append_statement(
            root_definition.entry_block(),
            MirStatement::new(
                root_origin,
                Some(MirPlace::temp(closure)),
                MirStatementKind::Allocate(MirAggregate::Closure {
                    function: child,
                    captures: Vec::new(),
                }),
                MirEffect::allocation(),
                Some(closure_safepoint),
            ),
        )
        .expect("the parent closure may name its reserved child");
    root_definition
        .set_terminator(
            root_definition.entry_block(),
            MirTerminator::new(
                root_origin,
                MirTerminatorKind::Return(Some(MirOperand::Temp(closure))),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("root terminator should be unique");

    let mut child_definition = function(child_body, child_owner, child_origin);
    child_definition
        .set_terminator(
            child_definition.entry_block(),
            MirTerminator::new(
                child_origin,
                MirTerminatorKind::Return(None),
                MirEffect::PURE,
                None,
            ),
        )
        .expect("child terminator should be unique");

    program
        .define_function(child, child_definition)
        .expect("a child may be defined while its parent remains reserved");
    assert!(program.function(child).is_some());
    assert!(program.function(root).is_none());
    assert_eq!(program.defined_len(), 1);
    assert_eq!(
        program
            .undefined_reservations()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        [root]
    );
    program
        .define_function(root, root_definition)
        .expect("the parent definition must match its reservation");

    assert!(!program.has_undefined_reservations());
    assert_eq!(program.undefined_reservations().count(), 0);
    assert_eq!(program.defined_len(), 2);
    assert_eq!(
        program.functions().map(|(id, _)| id).collect::<Vec<_>>(),
        [root, child]
    );
    let dump = program.dump();
    assert_eq!(dump, program.dump());
    assert!(dump.contains("alloc.closure f1"));
    assert!(!dump.contains("<undefined>"));
}

#[test]
fn reservation_rejects_duplicate_function_identity() {
    let body = HirBodyId::new(710);
    let duplicate_body = HirBodyId::new(711);
    let first_origin = origin(body, 72);
    let duplicate_origin = origin(duplicate_body, 73);
    let function_id = FunctionId::new(712);
    let mut program = MirProgram::new(MirTargetTable::default());
    program
        .reserve_function(body, MirFunctionOwner::Function(function_id), first_origin)
        .expect("first function identity should reserve successfully");

    assert_eq!(
        program.reserve_function(
            duplicate_body,
            MirFunctionOwner::Function(function_id),
            duplicate_origin,
        ),
        Err(MirBuildError::DuplicateMirFunctionId {
            function_id,
            origin: duplicate_origin,
        })
    );
}

#[test]
fn reservation_rejects_duplicate_owner_qualified_method_identity() {
    let first_body = HirBodyId::new(720);
    let duplicate_body = HirBodyId::new(721);
    let first_origin = origin(first_body, 74);
    let duplicate_origin = origin(duplicate_body, 75);
    let first = method_target(722, 723, 724, 725);
    let duplicate = method_target(726, 723, 724, 727);
    let mut program = MirProgram::new(MirTargetTable::default());
    let first_reservation = program
        .reserve_function(first_body, MirFunctionOwner::Method(first), first_origin)
        .expect("first method identity should reserve successfully");
    assert_eq!(
        program.function_by_id(first.function),
        Some(first_reservation)
    );
    assert_eq!(
        program.method_by_id(first.owner, first.method),
        Some(first_reservation)
    );
    assert_eq!(program.functions_for_body(first_body), [first_reservation]);
    assert!(program.function(first_reservation).is_none());

    assert_eq!(
        program.reserve_function(
            duplicate_body,
            MirFunctionOwner::Method(duplicate),
            duplicate_origin,
        ),
        Err(MirBuildError::DuplicateMirMethodId {
            owner: duplicate.owner,
            method_id: duplicate.method,
            origin: duplicate_origin,
        })
    );
}

#[test]
fn reservation_rejects_a_lambda_with_a_missing_parent() {
    let body = HirBodyId::new(730);
    let child_origin = origin(body, 76);
    let parent = MirFunctionId::from_index(99);
    let mut program = MirProgram::new(MirTargetTable::default());

    assert_eq!(
        program.reserve_function(
            body,
            MirFunctionOwner::Lambda {
                parent,
                expression: HirExprId::new(731),
            },
            child_origin,
        ),
        Err(MirBuildError::MissingMirFunction {
            function: parent,
            origin: child_origin,
        })
    );
}

#[test]
fn definition_rejects_an_unreserved_function_slot() {
    let body = HirBodyId::new(740);
    let definition_origin = origin(body, 77);
    let reservation = MirFunctionId::from_index(0);
    let mut program = MirProgram::new(MirTargetTable::default());

    assert_eq!(
        program.define_function(
            reservation,
            function(
                body,
                MirFunctionOwner::Function(FunctionId::new(741)),
                definition_origin,
            ),
        ),
        Err(MirBuildError::MissingMirFunctionReservation {
            function: reservation,
            origin: definition_origin,
        })
    );
}

#[test]
fn definition_rejects_a_second_body_for_one_reservation() {
    let body = HirBodyId::new(750);
    let function_id = FunctionId::new(751);
    let definition_origin = origin(body, 78);
    let owner = MirFunctionOwner::Function(function_id);
    let mut program = MirProgram::new(MirTargetTable::default());
    let reservation = program
        .reserve_function(body, owner.clone(), definition_origin)
        .expect("function reservation should succeed");
    program
        .define_function(
            reservation,
            function(body, owner.clone(), definition_origin),
        )
        .expect("first definition should fill the reserved slot");

    assert_eq!(
        program.define_function(reservation, function(body, owner, definition_origin),),
        Err(MirBuildError::MirFunctionAlreadyDefined {
            function: reservation,
            origin: definition_origin,
        })
    );
}

#[test]
fn definition_rejects_a_body_that_differs_from_its_reservation() {
    let reserved_body = HirBodyId::new(760);
    let actual_body = HirBodyId::new(761);
    let reserved_origin = origin(reserved_body, 79);
    let actual_origin = origin(actual_body, 80);
    let owner = MirFunctionOwner::Function(FunctionId::new(762));
    let mut program = MirProgram::new(MirTargetTable::default());
    let reservation = program
        .reserve_function(reserved_body, owner.clone(), reserved_origin)
        .expect("function reservation should succeed");

    let error = program
        .define_function(reservation, function(actual_body, owner, actual_origin))
        .expect_err("definition body must match its reservation");
    assert_eq!(
        error,
        MirBuildError::MirFunctionReservationBodyMismatch {
            function: reservation,
            expected: reserved_body,
            actual: actual_body,
            origin: actual_origin,
        }
    );
    assert_eq!(error.origin(), Some(actual_origin));
}

#[test]
fn definition_rejects_an_owner_that_differs_from_its_reservation() {
    let body = HirBodyId::new(770);
    let reserved_origin = origin(body, 81);
    let actual_origin = origin(body, 82);
    let expected = MirFunctionOwner::Function(FunctionId::new(771));
    let actual = MirFunctionOwner::Function(FunctionId::new(772));
    let mut program = MirProgram::new(MirTargetTable::default());
    let reservation = program
        .reserve_function(body, expected.clone(), reserved_origin)
        .expect("function reservation should succeed");

    let error = program
        .define_function(reservation, function(body, actual.clone(), actual_origin))
        .expect_err("definition owner must match its reservation");
    assert_eq!(
        error,
        MirBuildError::MirFunctionReservationOwnerMismatch {
            function: reservation,
            expected: Box::new(expected),
            actual: Box::new(actual),
            origin: actual_origin,
        }
    );
    assert_eq!(error.origin(), Some(actual_origin));
}
