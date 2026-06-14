---
title: "String And Bytes Methods"
description: "String and bytes method documentation for Vela."
---

Strings are valid UTF-8 text values. Bytes are raw byte sequences. The two
types intentionally have different indexing rules: String indexes are byte
offsets that must land on UTF-8 boundaries, while bytes indexes address raw
`u8` values.

## String Search And Transform

String helpers include `len`, `is_empty`, `contains`, `find`, `starts_with`,
`ends_with`, `strip_prefix`, `strip_suffix`, `to_upper`, `to_lower`, `trim`,
`trim_start`, `trim_end`, `replace`, `repeat`, and `slice`.

```vela
fn main() {
    let label = "  Quest.Gold ".trim().replace(".", "_").to_lower();
    let kind = label.slice(0, 5);
    let item = label.strip_prefix("quest_").unwrap_or("");
    return kind + ":" + item;
}
```

`find`, `strip_prefix`, and `strip_suffix` return `Option`.

## Splitting, Parsing, And Characters

`split`, `split_once`, `split_lines`, and `split_whitespace` produce arrays.
Parsing helpers return `Option` so invalid input can be handled without a VM
trap.

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

Use `chars` for Unicode scalar values and `bytes` for UTF-8 bytes.

```vela
fn main() {
    let first = "gold".chars().next().unwrap_or('\0');
    return first.to_string().to_upper();
}
```

## Bytes

Bytes support `len`, `is_empty`, `slice`, `get`, `read_u32_le`,
`read_u32_be`, `to_hex`, `iter`, and `values`. `bytes::from_hex` returns
`Result` because malformed hex text has a recoverable error message.

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

Out-of-bounds byte reads and invalid string slice boundaries are VM diagnostics,
not `Option`.
