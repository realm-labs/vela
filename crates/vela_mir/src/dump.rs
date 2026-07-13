use std::fmt::{self, Write};

use crate::{
    MirAggregate, MirAwaitOperation, MirCall, MirDynamicArgument, MirEffect, MirEvaluatedConstant,
    MirFormatPart, MirFunction, MirFunctionOwner, MirGlobalOperation, MirHostOperation,
    MirImmediate, MirIndexOperation, MirIteratorOperation, MirOperand, MirPatternPredicate,
    MirProgram, MirReflectionOperation, MirRvalue, MirScriptArgument, MirSourceNode,
    MirSourceOrigin, MirStatement, MirStatementKind, MirTerminator, MirTerminatorKind,
};

impl MirProgram {
    #[must_use]
    pub fn dump(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for MirProgram {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "mir {{")?;
        for (id, target) in self.targets().functions() {
            writeln!(formatter, "  target function#{} {target:?}", id.get())?;
        }
        for (owner, id, target) in self.targets().methods() {
            writeln!(
                formatter,
                "  target type#{} method#{} {target:?}",
                owner.get(),
                id.get()
            )?;
        }
        for (id, target) in self.targets().types() {
            writeln!(formatter, "  target type#{} {target:?}", id.get())?;
        }
        for (id, target) in self.targets().variants() {
            writeln!(formatter, "  target variant#{} {target:?}", id.get())?;
        }
        for (id, target) in self.targets().fields() {
            writeln!(formatter, "  target field#{} {target:?}", id.get())?;
        }
        for (id, target) in self.targets().globals() {
            writeln!(formatter, "  target global#{} {target:?}", id.get())?;
        }
        for (function_id, reservation) in self.reservations() {
            write!(formatter, "  fn {function_id} ")?;
            if let Some(function) = self.function(function_id) {
                write_function(formatter, function)?;
            } else {
                writeln!(
                    formatter,
                    "reserved body h{} owner {} {} <undefined>",
                    reservation.body().get(),
                    function_owner(reservation.owner()),
                    origin(reservation.origin()),
                )?;
            }
        }
        formatter.write_str("}\n")
    }
}

fn write_function(formatter: &mut fmt::Formatter<'_>, function: &MirFunction) -> fmt::Result {
    writeln!(
        formatter,
        "body h{} owner {} symbol={:?} {} {{",
        function.body().get(),
        function_owner(function.owner()),
        function.code_symbol(),
        origin(function.origin())
    )?;

    if let Some(return_contract) = function.return_contract() {
        writeln!(
            formatter,
            "    return: {:?} {}",
            return_contract.contract,
            origin(return_contract.origin)
        )?;
    }
    for (index, parameter) in function.parameters().iter().enumerate() {
        writeln!(
            formatter,
            "    param p{index}: {} -> {} kind={:?} contract={:?} default={:?} hir=l{} {}",
            parameter.name,
            parameter.storage,
            parameter.kind,
            parameter.contract,
            parameter.default_body.map(|body| body.get()),
            parameter.hir_local.get(),
            origin(parameter.origin)
        )?;
    }
    for (index, capture) in function.captures().iter().enumerate() {
        writeln!(
            formatter,
            "    capture c{index}: {} -> {} hir_capture={} source=l{} {}",
            capture.name,
            capture.storage,
            capture.capture.get(),
            capture.source_local.get(),
            origin(capture.origin)
        )?;
    }
    for (local_id, local) in function.locals() {
        writeln!(
            formatter,
            "    local {local_id}: {:?} {:?} {}",
            local.kind,
            local.value_type,
            origin(local.origin)
        )?;
    }
    for (temp_id, temp) in function.temps() {
        let definition = temp
            .definition()
            .map_or_else(|| "undef".to_owned(), |statement| statement.to_string());
        writeln!(
            formatter,
            "    temp {temp_id}: {:?} def={definition} {}",
            temp.value_type,
            origin(temp.origin)
        )?;
    }
    for (debug_id, debug) in function.debug_locals() {
        writeln!(
            formatter,
            "    debug {debug_id}: {} -> {} kind={:?} hir={:?} scope=h{} live={} {}",
            debug.name,
            debug.storage,
            debug.kind,
            debug.hir_local.map(|local| local.get()),
            debug.scope.get(),
            format_ids(debug.live_region.blocks.iter()),
            origin(debug.origin)
        )?;
    }
    for (guard_id, guard) in function.guards() {
        match &guard.context {
            Some(context) => writeln!(
                formatter,
                "    guard {guard_id}: {:?} at {:?} name={:?} {}",
                guard.assumption,
                context.location,
                context.debug_name,
                origin(guard.origin)
            )?,
            None => writeln!(
                formatter,
                "    guard {guard_id}: {:?} {}",
                guard.assumption,
                origin(guard.origin)
            )?,
        }
    }
    for (safepoint_id, safepoint) in function.safepoints() {
        writeln!(
            formatter,
            "    safepoint {safepoint_id}: live={:?} {}",
            safepoint.live_values,
            origin(safepoint.origin)
        )?;
    }
    for (block, values) in &function.liveness().block_live_in {
        writeln!(formatter, "    live.in {block}: {values:?}")?;
    }
    for (block, values) in &function.liveness().block_live_out {
        writeln!(formatter, "    live.out {block}: {values:?}")?;
    }
    for (statement, values) in &function.liveness().statement_live_before {
        writeln!(formatter, "    live.before {statement}: {values:?}")?;
    }
    for (statement, values) in &function.liveness().statement_live_after {
        writeln!(formatter, "    live.after {statement}: {values:?}")?;
    }
    for (block_id, block) in function.blocks() {
        writeln!(formatter, "    {block_id}:")?;
        for statement_id in block.statements() {
            let Some(statement) = function.statement(*statement_id) else {
                writeln!(formatter, "      {statement_id}: <missing>")?;
                continue;
            };
            write!(formatter, "      {statement_id}: ")?;
            write_statement(formatter, statement)?;
        }
        if let Some(terminator) = block.terminator() {
            write!(formatter, "      -> ")?;
            write_terminator(formatter, terminator)?;
        } else {
            writeln!(formatter, "      -> <unterminated>")?;
        }
    }
    formatter.write_str("  }\n")
}

fn function_owner(owner: &MirFunctionOwner) -> String {
    match owner {
        MirFunctionOwner::Function(id) => format!("function#{}", id.get()),
        MirFunctionOwner::Method(target) => format!(
            "method#{} function#{} owner#{} node#{}",
            target.method.get(),
            target.function.get(),
            target.owner.get(),
            target.node.get()
        ),
        MirFunctionOwner::Lambda { parent, expression } => {
            format!("lambda({parent}, e{})", expression.get())
        }
    }
}

fn write_statement(formatter: &mut fmt::Formatter<'_>, statement: &MirStatement) -> fmt::Result {
    if let Some(destination) = statement.destination {
        write!(formatter, "{} = ", place(destination))?;
    }
    match &statement.kind {
        MirStatementKind::Assign(value) => write!(formatter, "{}", rvalue(value))?,
        MirStatementKind::Unary { operation, operand } => {
            write!(formatter, "{operation:?} {}", operand_text(operand))?;
        }
        MirStatementKind::Binary {
            operation,
            left,
            right,
        } => write!(
            formatter,
            "{operation:?} {}, {}",
            operand_text(left),
            operand_text(right)
        )?,
        MirStatementKind::DynamicUnary { operation, operand } => {
            write!(formatter, "dyn.{operation:?} {}", operand_text(operand))?;
        }
        MirStatementKind::DynamicBinary {
            operation,
            left,
            right,
        } => write!(
            formatter,
            "dyn.{operation:?} {}, {}",
            operand_text(left),
            operand_text(right)
        )?,
        MirStatementKind::ContextualNumericBinary {
            operation,
            value,
            literal,
            literal_side,
        } => write!(
            formatter,
            "contextual.{operation:?} value={} literal={literal:?} side={literal_side:?}",
            operand_text(value)
        )?,
        MirStatementKind::IdentityCompare {
            operation,
            left,
            right,
        } => write!(
            formatter,
            "identity.{operation:?} {}, {}",
            operand_text(left),
            operand_text(right)
        )?,
        MirStatementKind::TupleField { tuple, index } => {
            write!(formatter, "tuple.field {}.{index}", operand_text(tuple))?;
        }
        MirStatementKind::ReadField { receiver, target } => {
            write!(
                formatter,
                "field.read {} {target:?}",
                operand_text(receiver)
            )?;
        }
        MirStatementKind::WriteField {
            receiver,
            target,
            value,
        } => write!(
            formatter,
            "field.write {} {target:?}, {}",
            operand_text(receiver),
            operand_text(value)
        )?,
        MirStatementKind::Index(operation) => write_index(formatter, operation)?,
        MirStatementKind::Global(operation) => write_global(formatter, operation)?,
        MirStatementKind::Allocate(aggregate) => write_aggregate(formatter, aggregate)?,
        MirStatementKind::FormatString { parts } => {
            formatter.write_str("format[")?;
            for (index, part) in parts.iter().enumerate() {
                if index > 0 {
                    formatter.write_str(", ")?;
                }
                match part {
                    MirFormatPart::Text(text) => write!(formatter, "text({text:?})")?,
                    MirFormatPart::Value(value) => {
                        write!(formatter, "value({})", operand_text(value))?
                    }
                }
            }
            formatter.write_char(']')?;
        }
        MirStatementKind::MaterializeConstant(value) => {
            write!(formatter, "const.materialize {}", evaluated_constant(value))?;
        }
        MirStatementKind::MakeRange {
            start,
            end,
            inclusive,
        } => write!(
            formatter,
            "range.make {}, {} inclusive={inclusive}",
            operand_text(start),
            operand_text(end)
        )?,
        MirStatementKind::Call(call) => write_call(formatter, call)?,
        MirStatementKind::Host(operation) => write_host(formatter, operation)?,
        MirStatementKind::Reflect(operation) => write_reflection(formatter, operation)?,
        MirStatementKind::GuardTrap { value, guard } => {
            write!(formatter, "guard.trap {} with {guard}", operand_text(value))?;
        }
        MirStatementKind::Iterator(operation) => write_iterator(formatter, operation)?,
    }
    writeln!(
        formatter,
        " [{}{}] {}",
        effect(statement.effect),
        statement
            .safepoint
            .map_or_else(String::new, |id| format!(", {id}")),
        origin(statement.origin)
    )
}

fn write_script_arguments(
    formatter: &mut fmt::Formatter<'_>,
    arguments: &[MirScriptArgument],
) -> fmt::Result {
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            formatter.write_str(", ")?;
        }
        write!(formatter, "p{}=", argument.parameter)?;
        match &argument.value {
            Some(value) => formatter.write_str(&operand_text(value))?,
            None => formatter.write_str("<missing>")?,
        }
    }
    Ok(())
}

fn write_dynamic_arguments(
    formatter: &mut fmt::Formatter<'_>,
    arguments: &[MirDynamicArgument],
) -> fmt::Result {
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            formatter.write_str(", ")?;
        }
        if let Some(name) = &argument.name {
            write!(formatter, "{name}=")?;
        }
        formatter.write_str(&operand_text(&argument.value))?;
    }
    Ok(())
}

fn write_global(formatter: &mut fmt::Formatter<'_>, operation: &MirGlobalOperation) -> fmt::Result {
    match operation {
        MirGlobalOperation::Read { global } => write!(formatter, "global.read #{}", global.get()),
    }
}

fn write_index(formatter: &mut fmt::Formatter<'_>, operation: &MirIndexOperation) -> fmt::Result {
    match operation {
        MirIndexOperation::Read { receiver, index } => write!(
            formatter,
            "index.read {}, {}",
            operand_text(receiver),
            index_key(index)
        ),
        MirIndexOperation::Write {
            receiver,
            index,
            value,
        } => write!(
            formatter,
            "index.write {}, {}, {}",
            operand_text(receiver),
            index_key(index),
            operand_text(value)
        ),
    }
}

fn index_key(index: &crate::MirIndexKey) -> String {
    match index {
        crate::MirIndexKey::Value(value) => operand_text(value),
        crate::MirIndexKey::ConstantString(value) => format!("const-string({value:?})"),
    }
}

fn write_aggregate(formatter: &mut fmt::Formatter<'_>, aggregate: &MirAggregate) -> fmt::Result {
    match aggregate {
        MirAggregate::Tuple(values) => write_operand_list(formatter, "alloc.tuple", values),
        MirAggregate::Array(values) => write_operand_list(formatter, "alloc.array", values),
        MirAggregate::SetFromArray { source } => {
            write!(formatter, "alloc.set.from_array {}", operand_text(source))
        }
        MirAggregate::Map(values) => {
            formatter.write_str("alloc.map[")?;
            for (index, (key, value)) in values.iter().enumerate() {
                if index > 0 {
                    formatter.write_str(", ")?;
                }
                write!(formatter, "{key:?} => {}", operand_text(value))?;
            }
            formatter.write_char(']')
        }
        MirAggregate::Record {
            type_id,
            shape,
            fields,
        } => {
            write!(
                formatter,
                "alloc.record type#{} shape#{} [",
                type_id.get(),
                shape.get()
            )?;
            write_fields(formatter, fields)?;
            formatter.write_char(']')
        }
        MirAggregate::DynamicRecord { type_name, fields } => {
            write!(formatter, "alloc.record.dynamic type={type_name:?} [")?;
            write_dynamic_fields(formatter, fields)?;
            formatter.write_char(']')
        }
        MirAggregate::Enum {
            type_id,
            variant,
            fields,
        } => {
            write!(
                formatter,
                "alloc.enum type#{} variant#{} [",
                type_id.get(),
                variant.get()
            )?;
            write_fields(formatter, fields)?;
            formatter.write_char(']')
        }
        MirAggregate::DynamicVariant {
            owner_name,
            variant_name,
            fields,
        } => {
            write!(
                formatter,
                "alloc.variant.dynamic owner={owner_name:?} variant={variant_name:?} ["
            )?;
            write_dynamic_fields(formatter, fields)?;
            formatter.write_char(']')
        }
        MirAggregate::Closure { function, captures } => {
            write!(formatter, "alloc.closure {function} ")?;
            write_operand_list(formatter, "captures", captures)
        }
    }
}

fn write_operand_list(
    formatter: &mut fmt::Formatter<'_>,
    name: &str,
    values: &[MirOperand],
) -> fmt::Result {
    write!(formatter, "{name}[")?;
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            formatter.write_str(", ")?;
        }
        formatter.write_str(&operand_text(value))?;
    }
    formatter.write_char(']')
}

fn write_fields(
    formatter: &mut fmt::Formatter<'_>,
    fields: &[(vela_def::FieldId, MirOperand)],
) -> fmt::Result {
    for (index, (field, value)) in fields.iter().enumerate() {
        if index > 0 {
            formatter.write_str(", ")?;
        }
        write!(formatter, "#{}={}", field.get(), operand_text(value))?;
    }
    Ok(())
}

fn write_dynamic_fields(
    formatter: &mut fmt::Formatter<'_>,
    fields: &[(String, MirOperand)],
) -> fmt::Result {
    for (index, (field, value)) in fields.iter().enumerate() {
        if index > 0 {
            formatter.write_str(", ")?;
        }
        write!(formatter, "{field:?}={}", operand_text(value))?;
    }
    Ok(())
}

fn write_call(formatter: &mut fmt::Formatter<'_>, call: &MirCall) -> fmt::Result {
    match call {
        MirCall::ScriptFunction {
            function,
            debug_name,
            signature,
            arguments,
            parameter_guards,
        } => {
            write!(
                formatter,
                "call script#{} name={debug_name:?} parameter_guards={parameter_guards:?} signature={signature:?}(",
                function.get(),
            )?;
            write_script_arguments(formatter, arguments)?;
        }
        MirCall::ScriptMethod {
            target,
            debug_name,
            receiver,
            signature,
            arguments,
        } => {
            write!(
                formatter,
                "call method#{} function#{} owner#{} name={debug_name:?} receiver={} signature={signature:?}(",
                target.method.get(),
                target.function.get(),
                target.owner.get(),
                operand_text(receiver)
            )?;
            write_script_arguments(formatter, arguments)?;
        }
        MirCall::CallableValue { callee, arguments } => {
            write!(formatter, "call closure({})(", operand_text(callee))?;
            write_operand_values(formatter, arguments)?;
        }
        MirCall::DynamicCallable { callee, arguments } => {
            write!(formatter, "call dynamic({})(", operand_text(callee))?;
            write_dynamic_arguments(formatter, arguments)?;
        }
        MirCall::NativeFunction {
            function,
            debug_name,
            signature,
            arguments,
        } => {
            write!(
                formatter,
                "call native#{} name={debug_name:?} signature={signature:?}(",
                function.get(),
            )?;
            write_operand_values(formatter, arguments)?;
        }
        MirCall::StdlibFunction {
            function,
            debug_name,
            signature,
            arguments,
        } => {
            write!(
                formatter,
                "call stdlib#{} name={debug_name:?} signature={signature:?}(",
                function.get(),
            )?;
            write_operand_values(formatter, arguments)?;
        }
        MirCall::ValueMethod {
            owner,
            method,
            debug_name,
            receiver,
            signature,
            arguments,
        } => {
            write!(
                formatter,
                "call value-method type#{} method#{} name={debug_name:?} receiver={} signature={signature:?}(",
                owner.get(),
                method.get(),
                operand_text(receiver)
            )?;
            write_operand_values(formatter, arguments)?;
        }
        MirCall::DynamicMethod {
            target,
            receiver,
            arguments,
        } => {
            write!(
                formatter,
                "call dynamic-method {} receiver={}(",
                target.member,
                operand_text(receiver)
            )?;
            write_dynamic_arguments(formatter, arguments)?;
        }
    }
    formatter.write_char(')')
}

fn write_operand_values(
    formatter: &mut fmt::Formatter<'_>,
    arguments: &[MirOperand],
) -> fmt::Result {
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            formatter.write_str(", ")?;
        }
        formatter.write_str(&operand_text(argument))?;
    }
    Ok(())
}

fn write_host(formatter: &mut fmt::Formatter<'_>, operation: &MirHostOperation) -> fmt::Result {
    match operation {
        MirHostOperation::Read { root, path } => write!(
            formatter,
            "host.read {} type#{} path={:?}",
            operand_text(root),
            path.root_type.runtime.get(),
            path.segments
        ),
        MirHostOperation::Write { root, path, value } => write!(
            formatter,
            "host.write {} type#{} path={:?}, {}",
            operand_text(root),
            path.root_type.runtime.get(),
            path.segments,
            operand_text(value)
        ),
        MirHostOperation::Mutate {
            root,
            path,
            operation,
            value,
        } => write!(
            formatter,
            "host.mutate.{operation:?} {} type#{} path={:?} value={}",
            operand_text(root),
            path.root_type.runtime.get(),
            path.segments,
            operand_text(value)
        ),
        MirHostOperation::Remove { root, path } => write!(
            formatter,
            "host.remove {} type#{} path={:?}",
            operand_text(root),
            path.root_type.runtime.get(),
            path.segments
        ),
        MirHostOperation::Call {
            root,
            path,
            target,
            arguments,
        } => {
            write!(
                formatter,
                "host.call {} type#{} path={:?} method#{}(",
                operand_text(root),
                path.root_type.runtime.get(),
                path.segments,
                target.runtime.get()
            )?;
            write_operand_values(formatter, arguments)?;
            formatter.write_char(')')
        }
    }
}

fn write_reflection(
    formatter: &mut fmt::Formatter<'_>,
    operation: &MirReflectionOperation,
) -> fmt::Result {
    match operation {
        MirReflectionOperation::Read {
            function,
            target,
            member,
        } => {
            write!(
                formatter,
                "reflect.read function#{} {}[{}]",
                function.get(),
                operand_text(target),
                operand_text(member)
            )
        }
        MirReflectionOperation::Write {
            function,
            target,
            member,
            value,
        } => write!(
            formatter,
            "reflect.write function#{} {}[{}], {}",
            function.get(),
            operand_text(target),
            operand_text(member),
            operand_text(value)
        ),
        MirReflectionOperation::Call {
            function,
            target,
            tail,
        } => {
            write!(
                formatter,
                "reflect.call function#{} {}(",
                function.get(),
                operand_text(target)
            )?;
            write_operand_values(formatter, tail)?;
            formatter.write_char(')')
        }
    }
}

fn write_iterator(
    formatter: &mut fmt::Formatter<'_>,
    operation: &MirIteratorOperation,
) -> fmt::Result {
    match operation {
        MirIteratorOperation::Create { iterable } => {
            write!(formatter, "iterator.create {}", operand_text(iterable))
        }
    }
}

fn write_terminator(formatter: &mut fmt::Formatter<'_>, terminator: &MirTerminator) -> fmt::Result {
    match &terminator.kind {
        MirTerminatorKind::AwaitCall {
            operation,
            destination,
            resume,
        } => {
            formatter.write_str("await ")?;
            match operation.as_ref() {
                MirAwaitOperation::Call(call) => write_call(formatter, call)?,
                MirAwaitOperation::Host(operation) => write_host(formatter, operation)?,
                MirAwaitOperation::Reflect(operation) => {
                    write_reflection(formatter, operation)?;
                }
            }
            write!(formatter, " -> {} resume {resume}", place(*destination))?;
        }
        MirTerminatorKind::Jump(target) => write!(formatter, "jump {target}")?,
        MirTerminatorKind::Branch {
            condition,
            then_block,
            else_block,
        } => write!(
            formatter,
            "branch {} -> {then_block}, {else_block}",
            operand_text(condition)
        )?,
        MirTerminatorKind::Switch {
            discriminant,
            cases,
            otherwise,
        } => write!(
            formatter,
            "switch {} {:?} otherwise {otherwise}",
            operand_text(discriminant),
            cases
        )?,
        MirTerminatorKind::GuardBranch {
            value,
            guard,
            passed,
            slow,
        } => write!(
            formatter,
            "guard.branch {} with {guard} -> passed {passed}, slow {slow}",
            operand_text(value)
        )?,
        MirTerminatorKind::TrySwitch {
            value,
            target,
            result,
            continuations,
            propagate,
            invalid,
            join,
        } => write!(
            formatter,
            "try.switch {} target={target:?} result={result} continuations={continuations:?} propagate={propagate} invalid={invalid} join={join}",
            operand_text(value)
        )?,
        MirTerminatorKind::IteratorNext {
            iterator,
            item,
            next,
            done,
        } => write!(
            formatter,
            "iterator.next {} -> {item}, next {next}, done {done}",
            operand_text(iterator)
        )?,
        MirTerminatorKind::RangeNext {
            cursor,
            end,
            exhausted,
            inclusive,
            item,
            mode,
            next,
            done,
        } => write!(
            formatter,
            "range.next cursor={cursor} end={} exhausted={exhausted} inclusive={inclusive} item={item} mode={mode:?} -> next {next}, done {done}",
            operand_text(end)
        )?,
        MirTerminatorKind::Return(value) => write!(
            formatter,
            "return {}",
            value
                .as_ref()
                .map_or_else(|| "unit".to_owned(), operand_text)
        )?,
        MirTerminatorKind::TryTypeMismatch { target } => {
            write!(formatter, "try.type-mismatch target={target:?}")?;
        }
        MirTerminatorKind::Unreachable => formatter.write_str("unreachable")?,
    }
    writeln!(
        formatter,
        " [{}{}] {}",
        effect(terminator.effect),
        terminator
            .safepoint
            .map_or_else(String::new, |id| format!(", {id}")),
        origin(terminator.origin)
    )
}

fn operand_text(operand: &MirOperand) -> String {
    match operand {
        MirOperand::Immediate(value) => immediate(*value),
        MirOperand::Local(id) => id.to_string(),
        MirOperand::Temp(id) => id.to_string(),
    }
}

fn place(place: crate::MirPlace) -> String {
    match place {
        crate::MirPlace::Local(id) => id.to_string(),
        crate::MirPlace::Temp(id) => id.to_string(),
    }
}

fn immediate(value: MirImmediate) -> String {
    match value {
        MirImmediate::Unit => "unit".to_owned(),
        MirImmediate::Bool(value) => value.to_string(),
        MirImmediate::Char(value) => format!("{value:?}"),
        MirImmediate::Scalar(value) => value.to_string(),
    }
}

fn evaluated_constant(value: &MirEvaluatedConstant) -> String {
    match value {
        MirEvaluatedConstant::Unit => "unit".to_owned(),
        MirEvaluatedConstant::Bool(value) => value.to_string(),
        MirEvaluatedConstant::Char(value) => format!("{value:?}"),
        MirEvaluatedConstant::Scalar(value) => value.to_string(),
        MirEvaluatedConstant::String(value) => format!("{value:?}"),
        MirEvaluatedConstant::Bytes(value) => format!("bytes{value:?}"),
        MirEvaluatedConstant::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(evaluated_constant)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        MirEvaluatedConstant::Map(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(key, value)| format!("{key:?}: {}", evaluated_constant(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn rvalue(value: &MirRvalue) -> String {
    match value {
        MirRvalue::Use(operand) => operand_text(operand),
        MirRvalue::Constant { value, provenance } => format!(
            "constant.{} {}",
            match provenance {
                crate::MirConstantProvenance::Literal => "literal",
                crate::MirConstantProvenance::FoldedLiteral => "folded-literal",
                crate::MirConstantProvenance::EvaluatedConstant => "evaluated",
                crate::MirConstantProvenance::PatternLiteral => "pattern-literal",
            },
            immediate(*value)
        ),
        MirRvalue::Truthy { value } => format!("truthy {}", operand_text(value)),
        MirRvalue::IsMissing { value } => format!("is_missing {}", operand_text(value)),
        MirRvalue::PatternPredicate(predicate) => pattern_predicate(predicate),
    }
}

fn pattern_predicate(predicate: &MirPatternPredicate) -> String {
    match predicate {
        MirPatternPredicate::TupleArity { value, arity } => {
            format!("pattern.tuple-arity {} == {arity}", operand_text(value))
        }
        MirPatternPredicate::NeverMatches { value } => {
            format!("pattern.never {}", operand_text(value))
        }
        MirPatternPredicate::VariantShape {
            value,
            type_id,
            variant,
        } => format!(
            "pattern.variant-shape {} type#{} variant#{}",
            operand_text(value),
            type_id.get(),
            variant.get()
        ),
        MirPatternPredicate::DynamicVariant {
            value,
            owner_name,
            variant_name,
        } => format!(
            "pattern.variant.dynamic {} owner={owner_name:?} variant={variant_name:?}",
            operand_text(value)
        ),
    }
}

fn effect(effect: MirEffect) -> String {
    if effect.is_pure() {
        return "pure".to_owned();
    }
    let mut names = Vec::new();
    for (name, enabled) in [
        ("trap", effect.may_trap),
        ("alloc", effect.may_allocate),
        ("script-call", effect.script_call),
        ("dynamic-call", effect.dynamic_call),
        ("global-read", effect.global_read),
        ("host-read", effect.host_read),
        ("host-write", effect.host_write),
        ("host-call", effect.host_call),
        ("reflect-read", effect.reflection_read),
        ("reflect-write", effect.reflection_write),
        ("reflect-call", effect.reflection_call),
        ("event", effect.emits_event),
        ("time", effect.reads_time),
        ("random", effect.uses_random),
        ("io-read", effect.reads_io),
        ("io-write", effect.writes_io),
    ] {
        if enabled {
            names.push(name);
        }
    }
    names.join("|")
}

fn origin(origin: MirSourceOrigin) -> String {
    let node = match origin.node {
        MirSourceNode::Declaration(id) => format!("d{}", id.get()),
        MirSourceNode::Body(id) => format!("h{}", id.get()),
        MirSourceNode::Expression(id) => format!("e{}", id.get()),
        MirSourceNode::Statement(id) => format!("s{}", id.get()),
        MirSourceNode::Pattern(id) => format!("p{}", id.get()),
    };
    format!(
        "@{}:{}..{}/{}",
        origin.span.source.get(),
        origin.span.start,
        origin.span.end,
        node
    )
}

fn format_ids<'a, T: fmt::Display + 'a>(ids: impl Iterator<Item = &'a T>) -> String {
    let mut output = String::from("[");
    for (index, id) in ids.enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        let _ = write!(output, "{id}");
    }
    output.push(']');
    output
}
