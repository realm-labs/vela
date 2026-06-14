---
title: "源码文件、目录和变更文件"
description: "Vela 宿主如何编译单文件和多文件更新。"
---

Vela 支持基于源码的 reload 工作流：宿主可以从单个文件、目录或变更文件
集合编译新程序版本。最终结果仍然是一个完整 candidate version，并且必须
通过兼容性检查。

## Source Identity

编译 API 内部使用 source label 和 source identity，让诊断、span、module
归属和 reload report 可以指回正确文件。普通文件或目录工作流中，应用层
用户不应该需要手写稳定 ID。

## 单文件

单文件更新适合示例、playground 和小型嵌入规则。它会作为完整 candidate
program 编译，然后 stage，并在 safe point 应用。

## 目录和变更文件

更大的项目应从项目根目录或可以还原完整 module graph 的变更集合编译。
一次更新多个 module 是可以的，前提是最终 graph 是一致的。

changed-file 工作流仍然要在推进当前版本前校验 imports、重复声明、module
可见性、顶层副作用限制、ABI、schema 和 effects。

## Source 边界拒绝

如果变更源码无法安全关联到现有 module graph，更新会被拒绝。典型情况包
括缺失 module、重复定义歧义、不兼容导出声明，或需要宿主批准的源码变
更。
