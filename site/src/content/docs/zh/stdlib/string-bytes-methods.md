---
title: "String 和 Bytes 方法"
description: "Vela String 和 Bytes 方法文档。"
---

String 是合法 UTF-8 文本值。Bytes 是原始字节序列。两者的索引规则不同：
string 索引是字节 offset，并且必须落在 UTF-8 边界上；bytes 索引直接访问
`u8`。

## String 搜索和转换

String helper 包括 `len`、`is_empty`、`contains`、`find`、
`starts_with`、`ends_with`、`strip_prefix`、`strip_suffix`、`to_upper`、
`to_lower`、`trim`、`trim_start`、`trim_end`、`replace`、`repeat` 和
`slice`。

```vela
fn main() {
    let label = "  Quest.Gold ".trim().replace(".", "_").to_lower();
    let kind = label.slice(0, 5);
    let item = label.strip_prefix("quest_").unwrap_or("");
    return kind + ":" + item;
}
```

`find`、`strip_prefix` 和 `strip_suffix` 返回 `Option`。

## Split、Parse 和 Char

`split`、`split_once`、`split_lines` 和 `split_whitespace` 产生数组。Parse
helper 返回 `Option`，所以无效输入可以由脚本处理，不会直接变成 VM trap。

```vela
fn main() {
    let parts = "count=3 enabled=true".split_whitespace();
    let count = parts[0].split_once("=").unwrap_or(["count", "0"])[1]
        .parse_i64()
        .unwrap_or(0);
    let enabled = parts[1].split_once("=").unwrap_or(["enabled", "false"])[1]
        .parse_bool()
        .unwrap_or(false);
    return enabled && count == 3;
}
```

Unicode scalar value 使用 `chars`，UTF-8 字节使用 `bytes`。

```vela
fn main() {
    let first = "gold".chars().next().unwrap_or('\0');
    return first.to_string().to_upper();
}
```

## Bytes

Bytes 支持 `len`、`is_empty`、`slice`、`get`、`read_u32_le`、
`read_u32_be`、`to_hex`、`iter` 和 `values`。`bytes::from_hex` 返回
`Result`，因为格式错误的 hex 文本有可恢复错误信息。

```vela
fn main() {
    let decoded = bytes::from_hex("01000000");
    let bytes = result::unwrap_or(decoded, b"");
    if bytes.len() >= 4 {
        return bytes.read_u32_le(0);
    }
    return 0;
}
```

越界 byte 读取和非法 string slice 边界是 VM diagnostic，不是 `Option`。
