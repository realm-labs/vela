# Collections

Vela has first-class arrays, maps, and sets. They are VM-owned values unless they are host paths.

## Arrays

```vela
let values = [1, 2, 3];
let doubled = values.map(|item| item * 2);
let large = doubled.filter(|item| item > 2);
```

## Maps

```vela
let rewards = { "gold": 10, "xp": 25 };

if rewards.has("gold") {
    rewards["gold"] += 5;
}
```

Maps use string keys in script-owned map literals.

## Sets

```vela
let tags = set::from_array(["daily", "vip", "daily"]);

if tags.has("vip") {
    return tags.len();
}
```

Higher-order methods execute callbacks synchronously. Capturing a host handle inside such a callback is valid during the same runtime call; using a saved callback after the host scope expires reports a stale handle error.
