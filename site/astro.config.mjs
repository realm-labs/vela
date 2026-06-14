import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import mdx from '@astrojs/mdx';

export default defineConfig({
  site: 'https://realm-labs.github.io',
  base: '/vela',
  integrations: [
    starlight({
      title: 'Vela',
      description: 'Hot Reload First scripting for Rust host-owned logic.',
      defaultLocale: 'root',
      locales: {
        root: {
          label: 'English',
          lang: 'en',
        },
        zh: {
          label: '中文',
          lang: 'zh-CN',
        },
      },
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/realm-labs/vela' },
      ],
      customCss: ['./src/styles/custom.css'],
      expressiveCode: {
        shiki: {
          langAlias: {
            vela: 'rust',
          },
        },
      },
      sidebar: [
    {
      label: "Guide",
      translations: { 'zh-CN': "指南" },
      items: [
        { label: "Overview", translations: { 'zh-CN': "概览" }, link: '/overview/' },
        { label: "Core Concepts", translations: { 'zh-CN': "核心概念" }, link: '/core-concepts/' },
        { label: "Quickstart", translations: { 'zh-CN': "快速开始" }, link: '/quickstart/' },
        { label: "Installation And CLI", translations: { 'zh-CN': "安装和 CLI" }, link: '/installation-cli/' },
        { label: "Playground", translations: { 'zh-CN': "Playground" }, link: '/playground/' },
        { label: "Examples And Cookbook", translations: { 'zh-CN': "示例和 Cookbook" }, link: '/examples-cookbook/' },
        { label: "Project Status And Roadmap", translations: { 'zh-CN': "项目状态和路线图" }, link: '/project-status-roadmap/' },
      ],
    },
    {
      label: "Language Basics",
      translations: { 'zh-CN': "语言基础" },
      items: [
        { label: "Lexical Structure And Comments", translations: { 'zh-CN': "词法结构和注释" }, link: '/language/lexical-structure-comments/' },
        { label: "Variables And Constants", translations: { 'zh-CN': "变量和常量" }, link: '/language/variables-constants/' },
        { label: "Primitive Values", translations: { 'zh-CN': "基础值" }, link: '/language/primitive-values/' },
        { label: "Type Hints And Guards", translations: { 'zh-CN': "类型提示和 Guard" }, link: '/language/type-hints-guards/' },
        { label: "Operators And Assignment", translations: { 'zh-CN': "运算符和赋值" }, link: '/language/operators-assignment/' },
        { label: "Control Flow", translations: { 'zh-CN': "控制流" }, link: '/language/control-flow/' },
        { label: "Functions", translations: { 'zh-CN': "函数" }, link: '/language/functions/' },
        { label: "Closures And Lambdas", translations: { 'zh-CN': "闭包和 Lambda" }, link: '/language/closures-lambdas/' },
        { label: "Modules And Imports", translations: { 'zh-CN': "模块和导入" }, link: '/language/modules-imports/' },
        { label: "Attributes", translations: { 'zh-CN': "属性" }, link: '/language/attributes/' },
      ],
    },
    {
      label: "Data Model",
      translations: { 'zh-CN': "数据模型" },
      items: [
        { label: "Records And Structs", translations: { 'zh-CN': "Record 和 Struct" }, link: '/data/records-structs/' },
        { label: "Enums And Match", translations: { 'zh-CN': "Enum 和 Match" }, link: '/data/enums-match/' },
        { label: "Arrays", translations: { 'zh-CN': "数组" }, link: '/data/arrays/' },
        { label: "Maps", translations: { 'zh-CN': "Map" }, link: '/data/maps/' },
        { label: "Sets", translations: { 'zh-CN': "Set" }, link: '/data/sets/' },
        { label: "Strings And Bytes", translations: { 'zh-CN': "String 和 Bytes" }, link: '/data/strings-bytes/' },
        { label: "Option And Result", translations: { 'zh-CN': "Option 和 Result" }, link: '/data/option-result/' },
        { label: "Ranges", translations: { 'zh-CN': "Range" }, link: '/data/ranges/' },
        { label: "Iterators And Sequences", translations: { 'zh-CN': "Iterator 和 Sequence" }, link: '/data/iterators-sequences/' },
      ],
    },
    {
      label: "Methods And Dispatch",
      translations: { 'zh-CN': "方法和分发" },
      items: [
        { label: "Inherent Methods", translations: { 'zh-CN': "固有方法" }, link: '/methods/inherent-methods/' },
        { label: "Traits And Trait Methods", translations: { 'zh-CN': "Trait 和 Trait 方法" }, link: '/methods/traits-trait-methods/' },
        { label: "Dynamic Method Dispatch", translations: { 'zh-CN': "动态方法分发" }, link: '/methods/dynamic-method-dispatch/' },
        { label: "Standard Library Methods", translations: { 'zh-CN': "标准库方法" }, link: '/methods/standard-library-methods/' },
      ],
    },
    {
      label: "Standard Library",
      translations: { 'zh-CN': "标准库" },
      items: [
        { label: "Standard Library Overview", translations: { 'zh-CN': "标准库概览" }, link: '/stdlib/overview/' },
        { label: "Array Methods", translations: { 'zh-CN': "数组方法" }, link: '/stdlib/array-methods/' },
        { label: "Map Methods", translations: { 'zh-CN': "Map 方法" }, link: '/stdlib/map-methods/' },
        { label: "Set Methods", translations: { 'zh-CN': "Set 方法" }, link: '/stdlib/set-methods/' },
        { label: "String And Bytes Methods", translations: { 'zh-CN': "String 和 Bytes 方法" }, link: '/stdlib/string-bytes-methods/' },
        { label: "Option And Result Methods", translations: { 'zh-CN': "Option 和 Result 方法" }, link: '/stdlib/option-result-methods/' },
        { label: "Math", translations: { 'zh-CN': "Math" }, link: '/stdlib/math/' },
        { label: "Time", translations: { 'zh-CN': "Time" }, link: '/stdlib/time/' },
        { label: "Random", translations: { 'zh-CN': "Random" }, link: '/stdlib/random/' },
        { label: "Context", translations: { 'zh-CN': "Context" }, link: '/stdlib/context/' },
        { label: "I/O", translations: { 'zh-CN': "I/O" }, link: '/stdlib/io/' },
      ],
    },
    {
      label: "Host Integration",
      translations: { 'zh-CN': "宿主集成" },
      items: [
        { label: "Embedding Overview", translations: { 'zh-CN': "嵌入概览" }, link: '/host/embedding-overview/' },
        { label: "Engine And Runtime", translations: { 'zh-CN': "Engine 和 Runtime" }, link: '/host/engine-runtime/' },
        { label: "Call Arguments And Return Values", translations: { 'zh-CN': "调用参数和返回值" }, link: '/host/call-arguments-return-values/' },
        { label: "Native Functions", translations: { 'zh-CN': "Native 函数" }, link: '/host/native-functions/' },
        { label: "Derive Macros", translations: { 'zh-CN': "Derive 宏" }, link: '/host/derive-macros/' },
        { label: "Host Types And Schemas", translations: { 'zh-CN': "Host 类型和 Schema" }, link: '/host/host-types-schemas/' },
        { label: "HostRef, HostPath, PathProxy", translations: { 'zh-CN': "HostRef、HostPath、PathProxy" }, link: '/host/hostref-hostpath-pathproxy/' },
        { label: "Host Object Lifetime", translations: { 'zh-CN': "Host 对象生命周期" }, link: '/host/host-object-lifetime/' },
        { label: "HostAccess Write-Through Model", translations: { 'zh-CN': "HostAccess 写穿模型" }, link: '/host/hostaccess-write-through/' },
        { label: "Runtime Globals", translations: { 'zh-CN': "Runtime Global" }, link: '/host/runtime-globals/' },
        { label: "Serde Snapshot Values", translations: { 'zh-CN': "Serde Snapshot 值" }, link: '/host/serde-snapshot-values/' },
        { label: "Capabilities And Execution Budgets", translations: { 'zh-CN': "能力和执行预算" }, link: '/host/capabilities-execution-budgets/' },
      ],
    },
    {
      label: "Hot Reload",
      translations: { 'zh-CN': "热更新" },
      items: [
        { label: "Hot Reload Model", translations: { 'zh-CN': "热更新模型" }, link: '/hot-reload/model/' },
        { label: "Runtime Update Workflow", translations: { 'zh-CN': "Runtime 更新流程" }, link: '/hot-reload/runtime-update-workflow/' },
        { label: "Safe Points", translations: { 'zh-CN': "Safe Point" }, link: '/hot-reload/safe-points/' },
        { label: "ABI And Schema Compatibility", translations: { 'zh-CN': "ABI 和 Schema 兼容性" }, link: '/hot-reload/abi-schema-compatibility/' },
        { label: "Source Files, Directories, And Changed Files", translations: { 'zh-CN': "源码文件、目录和变更文件" }, link: '/hot-reload/source-files-directories-changed-files/' },
        { label: "Rejection Reports", translations: { 'zh-CN': "拒绝报告" }, link: '/hot-reload/rejection-reports/' },
      ],
    },
    {
      label: "Reflection And Tooling",
      translations: { 'zh-CN': "反射和工具" },
      items: [
        { label: "Reflection Overview", translations: { 'zh-CN': "反射概览" }, link: '/reflection-tooling/reflection-overview/' },
        { label: "Metadata Queries", translations: { 'zh-CN': "元数据查询" }, link: '/reflection-tooling/metadata-queries/' },
        { label: "Controlled Reads, Writes, And Calls", translations: { 'zh-CN': "受控读写调用" }, link: '/reflection-tooling/controlled-reads-writes-calls/' },
        { label: "Diagnostics", translations: { 'zh-CN': "诊断" }, link: '/reflection-tooling/diagnostics/' },
        { label: "Completion And Editor Metadata", translations: { 'zh-CN': "补全和编辑器元数据" }, link: '/reflection-tooling/completion-editor-metadata/' },
        { label: "Fuzzing And Validation", translations: { 'zh-CN': "Fuzzing 和验证" }, link: '/reflection-tooling/fuzzing-validation/' },
        { label: "Performance And Benchmarks", translations: { 'zh-CN': "性能和 Benchmark" }, link: '/reflection-tooling/performance-benchmarks/' },
      ],
    },
    {
      label: "Reference",
      translations: { 'zh-CN': "参考" },
      items: [
        { label: "Grammar", translations: { 'zh-CN': "语法参考" }, link: '/reference/grammar/' },
        { label: "Engine API Reference", translations: { 'zh-CN': "Engine API 参考" }, link: '/reference/engine-api/' },
        { label: "C API", translations: { 'zh-CN': "C API" }, link: '/reference/c-api/' },
        { label: "Error And Diagnostic Codes", translations: { 'zh-CN': "错误和诊断码" }, link: '/reference/error-diagnostic-codes/' },
        { label: "Non-Goals And Constraints", translations: { 'zh-CN': "非目标和约束" }, link: '/reference/non-goals-constraints/' },
      ],
    }
      ],
    }),
    mdx(),
  ],
});
