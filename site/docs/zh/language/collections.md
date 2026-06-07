# 集合

Vela 有一等 array、map 和 set。除非它们是 host path，否则都是 VM-owned 值。

## Array

```vela
let values = [1, 2, 3];
let doubled = values.map(|item| item * 2);
let large = doubled.filter(|item| item > 2);
```

## Map

```vela
let rewards = { "gold": 10, "xp": 25 };

if rewards.has("gold") {
    rewards["gold"] += 5;
}
```

脚本 owned map literal 使用字符串 key。

## Set

```vela
let tags = set::from_array(["daily", "vip", "daily"]);

if tags.has("vip") {
    return tags.len();
}
```

高阶方法同步执行 callback。同一次 runtime call 内捕获 host handle 是合法的；如果保存 callback 后在 host scope 过期后访问，会报 stale handle。
