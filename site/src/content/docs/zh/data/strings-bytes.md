---
title: "String 和 Bytes"
description: "Vela String 和 Bytes文档。"
---

String 是 UTF-8 文本值。Bytes 是不可变二进制 buffer。Vela 明确区分两者，让文本 API 和二进制 API 在宿主边界和标准库契约中保持清晰。

## 字符串形式

普通字符串使用 `"..."`，多行字符串使用 `"""..."""`，插值字符串必须显式加 `f` 前缀。普通字符串不会插值。

```vela
fn greeting(name: string) -> string {
    return f"hello {name}"
}

fn template() -> string {
    return """
line one
line two
"""
}
```

## 文本方法

String 方法覆盖谓词、转换、查找、split、parse helper 和显式遍历。`len()`、`find()`、`slice(start, end)` 使用 byte index；`chars()` 是 UTF-8 字符遍历。

```vela
fn parse_count(text: string) {
    return text.trim().parse_i64()
}
```

## Bytes

Byte string 使用 `b"..."`。索引 bytes 值会得到 `u8`。Byte API 应使用显式 endian helper，而不是宿主 endian 读取。

```vela
fn first_byte(packet: bytes) -> u8 {
    return packet[0]
}
```

## 边界

String 和 bytes 都是堆上的运行时值。宿主 API 应明确声明它需要文本还是二进制数据；Vela 不会在两者之间静默转换。
