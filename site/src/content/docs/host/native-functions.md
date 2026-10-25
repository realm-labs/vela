---
title: "Native Functions"
description: "The synchronous, script-owned Vela boundary."
---

Existing independent synchronous Rust native functions accept and return values. They do not expose borrowed roots, transparent fields, retained views, or Host objects. Rust may borrow internally during one invocation, but cannot return that borrow as a script value.

The retained `native_function` example registers a typed scalar function and existing exported scalar/collection functions.

```vela
fn main() {
    let manual = game::bonus_manual(3, 4);
    let generated = game::bonus_macro(10, 7);
    let collection = game::collection_bonus({ "quest": 5, "raid": 8 }, ["vip", "daily"]);
    return manual + generated + collection;
}
```

Native permissions, effects, conversion errors and execution budgets remain enforced. There are no script futures, borrowed signatures, host constructors or replacement opaque methods.
