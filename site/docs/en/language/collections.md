# Collections

Vela has first-class arrays, maps, and sets. They are VM-owned values unless they are host paths.

## Arrays

```vela
let values = [1, 2, 3];
let collected = values.iter()
    .filter(|item| item > 1)
    .map(|item| item * 10)
    .collect_array();
let has_large = values.iter().any(|item| item > 2);
```

Retained eager helpers such as `values.map(...)` and `values.filter(...)` are convenience wrappers over the same callback, budget, and host-access semantics.

## Maps

```vela
let rewards = { "gold": 10, "xp": 25 };

if rewards.has("gold") {
    rewards["gold"] += 5;
}
```

Maps use string keys in script-owned map literals.

Map traversal is explicit:

```vela
let keys = rewards.keys().collect_array();
let amounts = rewards.values().collect_array();
let entries = rewards.entries().collect_array();
let large_rewards = rewards.values()
    .filter(|amount| amount >= 10)
    .collect_array();
```

## Sets

```vela
let tags = set::from_array(["daily", "vip", "daily"]);

if tags.has("vip") {
    return tags.len();
}
```

## Iterators And Sequences

Arrays, maps, sets, strings, and ranges are repeatable sequences: each `for in` traversal or `.iter()` call creates a fresh iterator. Iterator values are one-shot cursors. Calling `next()` or using an iterator in `for in` consumes that cursor.

```vela
let values = [1, 2, 3];
let iter = values.iter();

let first = iter.next();
let rest = iter.collect_array();
```

Core lazy adapters are `map`, `filter`, `take`, and `skip`. Terminal methods include `next`, `count`, `any`, `all`, `find`, and `collect_array`.

Strings are UTF-8 strings. `text.len()` and `text.slice(start, end)` use byte indexes. Use `text.chars()` for `char` values and `text.bytes()` for UTF-8 bytes as `u8`.

```vela
let chars = "a奖励".chars().collect_array();
let bytes = "a".bytes().collect_array();
```

Higher-order methods execute callbacks synchronously. Capturing a host handle inside such a callback is valid during the same runtime call; using a saved callback after the host scope expires reports a stale handle error.
