# 集合

Vela 有一等 array、map 和 set。除非它们是 host path，否则都是 VM-owned 值。

## Array

```vela
let values = [1, 2, 3];
let doubled = values.map(|item| item * 2);
let large = doubled.filter(|item| item > 2);
let lazy = values.iter().filter(|item| item > 1).map(|item| item * 10);
let collected = lazy.collect_array();
```

## Map

```vela
let rewards = { "gold": 10, "xp": 25 };

if rewards.has("gold") {
    rewards["gold"] += 5;
}
```

脚本 owned map literal 使用字符串 key。

Map 遍历是显式的：

```vela
let keys = rewards.keys().collect_array();
let amounts = rewards.values().collect_array();
let entries = rewards.entries().collect_array();
```

## Set

```vela
let tags = set::from_array(["daily", "vip", "daily"]);

if tags.has("vip") {
    return tags.len();
}
```

## Iterator 和 Sequence

Array、map、set、string 和 range 是可重复遍历的 sequence：每次 `for in` 或 `.iter()` 都会创建新的 iterator。Iterator value 是一次性 cursor。调用 `next()` 或用它执行 `for in` 会消耗这个 cursor。

```vela
let values = [1, 2, 3];
let iter = values.iter();

let first = iter.next();
let rest = iter.collect_array();
```

核心 lazy adapter 是 `map`、`filter`、`take` 和 `skip`。Terminal method 包括 `next`、`count`、`any`、`all`、`find` 和 `collect_array`。

String 是 UTF-8 字符串。`text.len()` 和 `text.slice(start, end)` 使用 byte index。用 `text.chars()` 遍历 `char`，用 `text.bytes()` 遍历 UTF-8 byte，结果是 `u8`。

```vela
let chars = "a奖励".chars().collect_array();
let bytes = "a".bytes().collect_array();
```

高阶方法同步执行 callback。同一次 runtime call 内捕获 host handle 是合法的；如果保存 callback 后在 host scope 过期后访问，会报 stale handle。
