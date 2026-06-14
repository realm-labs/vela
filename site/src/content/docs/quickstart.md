---
title: "Quickstart"
description: "Quickstart documentation for Vela."
---

The fastest way to try Vela is the Playground. Pick an example, edit the source, and run `main`.

```vela
fn main() {
    let rewards = { "gold": 10, "xp": 25 };
    return rewards["gold"] + rewards["xp"];
}
```

Rust embedding starts with an Engine compiling source, a Runtime owning execution state, and explicit call arguments and budgets for script entry calls.
