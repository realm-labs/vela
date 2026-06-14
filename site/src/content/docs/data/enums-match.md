---
title: "Enums And Match"
description: "Enums And Match documentation for Vela."
---

Enums model tagged values. Variants can be empty, tuple-like, or record-like, and `match` gives scripts a controlled way to branch on those shapes.

## Declaring Enums

Enum variants may carry fields and type hints. Vela does not use generic enum syntax, so `Option` and `Result` are ordinary dynamic enum families rather than `Option<T>` or `Result<T, E>`.

```vela
enum QuestState {
    NotStarted
    Active { step: i64 }
    Complete(reward: string)
}
```

## Matching

Patterns include wildcards, bindings, literals, paths, tuple variants, and record variants. A guard can refine an arm.

```vela
fn next_step(state: QuestState) -> i64 {
    match state {
        QuestState::Active { step } if step < 10 => step + 1,
        QuestState::Active { step } => step,
        _ => 0,
    }
}
```

## Variant Data

Record variants keep named fields. Tuple variants keep positional fields. Pattern bindings introduce local values for the arm body; updating an enum usually means constructing a new enum value.

## Reload And Reflection

Variant names, field shapes, hints, and stable IDs participate in reflection and hot reload checks. Removing or reshaping a public variant can be rejected because existing callers or active frames may still depend on it.
