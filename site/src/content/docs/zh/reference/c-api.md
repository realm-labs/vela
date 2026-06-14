---
title: "C API"
description: "Vela 外部 C ABI 表面的当前状态。"
---

`vela_c_api` crate 是非 Rust 宿主的外部二进制接口。它和 hot reload ABI 有
意分离：C ABI 描述 native embedding symbols，hot reload ABI 描述脚本兼
容性。

## 当前表面

第一版 slice 暴露：

```text
opaque engine handles
opaque runtime handles
API version query
source compilation
无参数 entry call
scalar result values
ABI-owned string/value cleanup
```

导出的 status code 区分 null pointer、invalid UTF-8、engine、compile、
runtime、unsupported value 和 panic failures。

## Value 所有权

通过 C ABI 返回的字符串和 byte buffer 由 ABI 调用者持有，直到用匹配的
Vela cleanup function 释放。opaque engine/runtime handles 也必须用对应
free function 释放。

## 边界

C API 不暴露 Rust references。未来 host object vtables 和 aggregate value
handles 也应保持同样安全规则：宿主修改必须跨过明确 adapter 边界。

## 状态

C API 仍处于早期并且有意保持很小。在 ABI versioning 和 release hardening
完成前，本页应视为能力概要。
