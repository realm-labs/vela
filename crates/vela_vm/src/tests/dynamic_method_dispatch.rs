use super::linked_standard_method_cache_support::RecordingMethodCaches;
use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn linked_dynamic_string_predicate_resolves_by_runtime_receiver() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main(value) {
    return value.starts_with("q");
}
"#,
    )
    .expect("dynamic method source should compile");
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_program_with_budget(
            &Vm::new(),
            &program,
            "main",
            &[OwnedValue::String("quest".to_owned())],
            &mut budget,
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_linked_test_program_with_budget(
            &Vm::new(),
            &program,
            "main",
            &[OwnedValue::String("raid".to_owned())],
            &mut budget,
        ),
        Ok(OwnedValue::Bool(false))
    );
}

#[test]
fn linked_dynamic_standard_value_methods_resolve_representative_receivers() {
    assert_eq!(
        run_dynamic_method_source(
            r#"
fn main(value) {
    return value.trim();
}
"#,
            &[OwnedValue::String(" quest ".to_owned())],
        ),
        Ok(OwnedValue::String("quest".to_owned()))
    );
    assert_eq!(
        run_dynamic_method_source(
            r#"
fn main(value) {
    return value.len();
}
"#,
            &[OwnedValue::array([OwnedValue::i64(1), OwnedValue::i64(2)])],
        ),
        Ok(OwnedValue::i64(2))
    );
    assert_eq!(
        run_dynamic_method_source(
            r#"
fn main(value) {
    return value.get("level");
}
"#,
            &[OwnedValue::map([("level", OwnedValue::i64(7))])],
        ),
        Ok(OwnedValue::enum_variant(
            "Option",
            "Some",
            [("0", OwnedValue::i64(7))]
        ))
    );
    assert_eq!(
        run_dynamic_method_source(
            r#"
fn main(value) {
    return value.is_some();
}
"#,
            &[OwnedValue::enum_variant(
                "Option",
                "Some",
                [("0", OwnedValue::i64(3))]
            )],
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_dynamic_method_source(
            r#"
fn main(value) {
    return value.is_ok();
}
"#,
            &[OwnedValue::enum_variant(
                "Result",
                "Ok",
                [("0", OwnedValue::i64(3))]
            )],
        ),
        Ok(OwnedValue::Bool(true))
    );
}

#[test]
fn linked_dynamic_standard_value_method_errors_keep_source_span() {
    let error = run_dynamic_method_source(
        r#"
fn main(value) {
    return value.starts_with("q");
}
"#,
        &[OwnedValue::i64(42)],
    )
    .expect_err("integer receiver should not support starts_with");

    assert!(matches!(
        error.kind(),
        VmErrorKind::UnknownMethod { method } if method == "starts_with"
    ));
    assert!(
        error.source_span.is_some(),
        "dynamic missing method error should keep the call span"
    );

    let error = run_dynamic_method_source(
        r#"
fn main(value) {
    return value.starts_with();
}
"#,
        &[OwnedValue::String("quest".to_owned())],
    )
    .expect_err("missing dynamic argument should fail at runtime");

    assert!(matches!(error.kind(), VmErrorKind::ArityMismatch { .. }));
    assert!(
        error.source_span.is_some(),
        "dynamic arity error should keep the call span"
    );

    let error = run_dynamic_method_source(
        r#"
fn main(value) {
    return value.starts_with(42);
}
"#,
        &[OwnedValue::String("quest".to_owned())],
    )
    .expect_err("wrong dynamic argument type should fail at runtime");

    assert!(matches!(
        error.kind(),
        VmErrorKind::TypeMismatch { operation } if operation == "method starts_with"
    ));
    assert!(
        error.source_span.is_some(),
        "dynamic argument type error should keep the call span"
    );
}

#[test]
fn linked_dynamic_method_call_reports_source_spanned_unknown_method_for_unsupported_receiver() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main(value) {
    return value.starts_with("q");
}
"#,
    )
    .expect("dynamic method source should compile");
    let mut budget = ExecutionBudget::unbounded();

    let error = run_linked_test_program_with_budget(
        &Vm::new(),
        &program,
        "main",
        &[OwnedValue::i64(42)],
        &mut budget,
    )
    .expect_err("unsupported dynamic receiver should fail at runtime");

    assert!(matches!(
        error.kind(),
        VmErrorKind::UnknownMethod { method } if method == "starts_with"
    ));
    assert!(
        error.source_span.is_some(),
        "dynamic method runtime error should keep the call span"
    );
}

#[test]
fn linked_dynamic_script_method_resolves_by_runtime_receiver_type() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
struct Label {
    text: string,
}

impl Label {
    fn starts_with(self, prefix: string) -> bool {
        return self.text.starts_with(prefix);
    }
}

fn f(x) {
    return x.starts_with("q");
}

fn quest() {
    return f(Label { text: "quest" });
}

fn raid() {
    return f(Label { text: "raid" });
}

fn bad() {
    return f(42);
}
"#,
    )
    .expect("dynamic script method source should compile");

    assert_eq!(
        run_dynamic_entry(&program, "quest"),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_dynamic_entry(&program, "raid"),
        Ok(OwnedValue::Bool(false))
    );

    let error = run_dynamic_entry(&program, "bad")
        .expect_err("unsupported dynamic script method receiver should fail");
    assert!(matches!(
        error.kind(),
        VmErrorKind::UnknownMethod { method } if method == "starts_with"
    ));
    assert!(
        error.source_span.is_some(),
        "dynamic script method failure should keep the call span"
    );
}

#[test]
fn linked_dynamic_script_method_handles_heterogeneous_receivers() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
struct Label {
    text: string,
}

impl Label {
    fn starts_with(self, prefix: string) -> bool {
        return self.text.starts_with(prefix);
    }
}

fn main() {
    let values = [
        Label { text: "quest" },
        Label { text: "raid" },
        "quick",
    ];

    let count = 0;
    for value in values {
        if value.starts_with("q") {
            count += 1;
        }
    }

    return count;
}
"#,
    )
    .expect("heterogeneous dynamic script method source should compile");

    assert_eq!(run_dynamic_entry(&program, "main"), Ok(OwnedValue::i64(2)));
}

#[test]
fn linked_dynamic_script_method_materializes_named_and_default_args_after_resolution() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
struct Label {
    text: string,
}

impl Label {
    fn wrap(self, prefix: string = "[", suffix: string = "]") -> string {
        return [prefix, self.text, suffix].join("");
    }
}

fn wrap_named(x) {
    return x.wrap(suffix = "}", prefix = "{");
}

fn wrap_defaulted(x) {
    return x.wrap();
}

fn named_case() {
    return wrap_named(Label { text: "quest" });
}

fn defaulted_case() {
    return wrap_defaulted(Label { text: "quest" });
}
"#,
    )
    .expect("dynamic script method named/default source should compile");

    assert_eq!(
        run_dynamic_entry(&program, "named_case"),
        Ok(OwnedValue::String("{quest}".to_owned()))
    );
    assert_eq!(
        run_dynamic_entry(&program, "defaulted_case"),
        Ok(OwnedValue::String("[quest]".to_owned()))
    );
}

#[test]
fn linked_dynamic_script_method_named_arg_errors_keep_source_span() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
struct Label {
    text: string,
}

impl Label {
    fn wrap(self, prefix: string = "[", suffix: string = "]") -> string {
        return [prefix, self.text, suffix].join("");
    }

    fn require(self, prefix: string) -> string {
        return [prefix, self.text].join("");
    }
}

fn missing_required(x) {
    return x.require();
}

fn unknown_named(x) {
    return x.wrap(extra = "!");
}

fn unsupported_receiver(x) {
    return x.wrap(prefix = "{");
}

fn missing_case() {
    return missing_required(Label { text: "quest" });
}

fn unknown_case() {
    return unknown_named(Label { text: "quest" });
}

fn unsupported_case() {
    return unsupported_receiver(42);
}
"#,
    )
    .expect("dynamic script method named/default error source should compile");

    let error = run_dynamic_entry(&program, "missing_case")
        .expect_err("missing required dynamic script method argument should fail");
    assert!(matches!(error.kind(), VmErrorKind::ArityMismatch { .. }));
    assert!(
        error.source_span.is_some(),
        "missing dynamic method argument error should keep the call span"
    );

    let error = run_dynamic_entry(&program, "unknown_case")
        .expect_err("unknown named dynamic script method argument should fail");
    assert!(
        matches!(error.kind(), VmErrorKind::TypeMismatch { operation }
        if operation == "dynamic method unknown named argument")
    );
    assert!(
        error.source_span.is_some(),
        "unknown dynamic method named argument error should keep the call span"
    );

    let error = run_dynamic_entry(&program, "unsupported_case")
        .expect_err("unsupported dynamic receiver with named args should fail");
    assert!(matches!(
        error.kind(),
        VmErrorKind::UnknownMethod { method } if method == "wrap"
    ));
    assert!(
        error.source_span.is_some(),
        "unsupported receiver dynamic method error should keep the call span"
    );
}

#[test]
fn linked_dynamic_method_cache_hits_same_standard_receiver() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main(value) {
    return value.starts_with("q");
}
"#,
    )
    .expect("dynamic std cache source should compile");
    let linked = link_test_program(&program);
    let site = linked_dynamic_method_site(&linked, "main");
    let caches = RecordingMethodCaches::new(linked_cache_len(&linked));
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_entry_with_caches(
            &Vm::new(),
            &linked,
            "main",
            &[OwnedValue::String("quest".to_owned())],
            &mut budget,
            &caches,
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(caches.dynamic_set_count_for(site), 1);
    assert!(matches!(
        caches.dynamic_entry(site).map(|entry| entry.receiver_guard),
        Some(DynamicReceiverGuard::StdValue {
            receiver: StandardMethodReceiver::String,
        })
    ));

    assert_eq!(
        run_linked_test_entry_with_caches(
            &Vm::new(),
            &linked,
            "main",
            &[OwnedValue::String("raid".to_owned())],
            &mut budget,
            &caches,
        ),
        Ok(OwnedValue::Bool(false))
    );
    assert_eq!(caches.dynamic_get_count_for(site), 2);
    assert_eq!(caches.dynamic_set_count_for(site), 1);
}

#[test]
fn linked_dynamic_method_cache_resolves_iterator_adapter_targets() {
    assert_dynamic_iterator_adapter_cache(
        "take",
        StandardMethodInlineCacheTarget::Take,
        OwnedValue::array([OwnedValue::i64(1), OwnedValue::i64(2)]),
    );
    assert_dynamic_iterator_adapter_cache(
        "skip",
        StandardMethodInlineCacheTarget::Skip,
        OwnedValue::array([OwnedValue::i64(3), OwnedValue::i64(4)]),
    );
}

#[test]
fn linked_dynamic_method_cache_guard_miss_resolves_script_after_standard_receiver() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
struct Label {
    text: string,
}

impl Label {
    fn starts_with(self, prefix: string) -> bool {
        return self.text.starts_with(prefix);
    }
}

fn main(value) {
    return value.starts_with("q");
}
"#,
    )
    .expect("dynamic std-to-script cache source should compile");
    let linked = link_test_program(&program);
    let site = linked_dynamic_method_site(&linked, "main");
    let caches = RecordingMethodCaches::new(linked_cache_len(&linked));
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_entry_with_caches(
            &Vm::new(),
            &linked,
            "main",
            &[OwnedValue::String("quest".to_owned())],
            &mut budget,
            &caches,
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_linked_test_entry_with_caches(
            &Vm::new(),
            &linked,
            "main",
            &[OwnedValue::record(
                "Label",
                [("text", OwnedValue::String("raid".to_owned()))],
            )],
            &mut budget,
            &caches,
        ),
        Ok(OwnedValue::Bool(false))
    );
    assert_eq!(caches.dynamic_set_count_for(site), 2);
    assert!(matches!(
        caches.dynamic_entry(site).map(|entry| entry.receiver_guard),
        Some(DynamicReceiverGuard::ScriptType {
            ref type_name,
            ..
        }) if type_name == "Label"
    ));

    assert_eq!(
        run_linked_test_entry_with_caches(
            &Vm::new(),
            &linked,
            "main",
            &[OwnedValue::record(
                "Label",
                [("text", OwnedValue::String("quest".to_owned()))],
            )],
            &mut budget,
            &caches,
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(caches.dynamic_set_count_for(site), 2);
}

#[test]
fn linked_dynamic_method_cache_guard_misses_between_script_receiver_types() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
struct LabelA {
    text: string,
}

struct LabelB {
    text: string,
}

impl LabelA {
    fn starts_with(self, prefix: string) -> bool {
        return self.text.starts_with(prefix);
    }
}

impl LabelB {
    fn starts_with(self, prefix: string) -> bool {
        return self.text.starts_with(prefix);
    }
}

fn main(value) {
    return value.starts_with("q");
}
"#,
    )
    .expect("dynamic script guard miss source should compile");
    let linked = link_test_program(&program);
    let site = linked_dynamic_method_site(&linked, "main");
    let caches = RecordingMethodCaches::new(linked_cache_len(&linked));
    let mut budget = ExecutionBudget::unbounded();

    assert_eq!(
        run_linked_test_entry_with_caches(
            &Vm::new(),
            &linked,
            "main",
            &[OwnedValue::record(
                "LabelA",
                [("text", OwnedValue::String("quest".to_owned()))],
            )],
            &mut budget,
            &caches,
        ),
        Ok(OwnedValue::Bool(true))
    );
    assert_eq!(
        run_linked_test_entry_with_caches(
            &Vm::new(),
            &linked,
            "main",
            &[OwnedValue::record(
                "LabelB",
                [("text", OwnedValue::String("raid".to_owned()))],
            )],
            &mut budget,
            &caches,
        ),
        Ok(OwnedValue::Bool(false))
    );
    assert_eq!(caches.dynamic_set_count_for(site), 2);
    assert!(matches!(
        caches.dynamic_entry(site).map(|entry| entry.receiver_guard),
        Some(DynamicReceiverGuard::ScriptType {
            ref type_name,
            ..
        }) if type_name == "LabelB"
    ));
}

#[test]
fn linked_dynamic_method_rejects_undersized_inline_cache_provider() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main(value) {
    return value.starts_with("q");
}
"#,
    )
    .expect("dynamic cache layout source should compile");
    let linked = link_test_program(&program);
    let caches = RecordingMethodCaches::new(0);
    let mut budget = ExecutionBudget::unbounded();
    let error = run_linked_test_entry_with_caches(
        &Vm::new(),
        &linked,
        "main",
        &[OwnedValue::String("quest".to_owned())],
        &mut budget,
        &caches,
    )
    .expect_err("undersized cache provider should be rejected before execution");

    assert!(matches!(
        error.kind(),
        VmErrorKind::InlineCacheLayoutMismatch { .. }
    ));
    assert_eq!(caches.dynamic_set_count(), 0);
}

fn run_dynamic_method_source(source: &str, args: &[OwnedValue]) -> VmResult<OwnedValue> {
    let program = compile_standard_program_source(SourceId::new(1), source)
        .expect("dynamic method source should compile");
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_budget(&Vm::new(), &program, "main", args, &mut budget)
}

fn run_dynamic_entry(program: &UnlinkedProgram, entry: &str) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_budget(&Vm::new(), program, entry, &[], &mut budget)
}

fn assert_dynamic_iterator_adapter_cache(
    method: &str,
    target: StandardMethodInlineCacheTarget,
    expected: OwnedValue,
) {
    let source = format!(
        r#"
fn adapt(value) {{
    return value.{method}(2);
}}

fn main() {{
    return adapt([1, 2, 3, 4].iter()).collect_array();
}}
"#
    );
    let program = compile_standard_program_source(SourceId::new(1), &source)
        .expect("dynamic iterator adapter source should compile");
    let linked = link_test_program(&program);
    let site = linked_dynamic_method_site(&linked, "adapt");
    let caches = RecordingMethodCaches::new(linked_cache_len(&linked));
    let mut budget = ExecutionBudget::unbounded();

    let actual =
        run_linked_test_entry_with_caches(&Vm::new(), &linked, "main", &[], &mut budget, &caches);
    assert_eq!(actual, Ok(expected.clone()));
    let entry = caches
        .dynamic_entry(site)
        .expect("dynamic iterator adapter cache should populate");
    assert!(matches!(
        entry.receiver_guard,
        DynamicReceiverGuard::StdValue {
            receiver: StandardMethodReceiver::Iterator,
        }
    ));
    let DynamicMethodInlineCacheTarget::StandardValue {
        method_id,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("dynamic iterator adapter should resolve to a standard value target");
    };
    assert_eq!(
        method_id,
        vela_stdlib::std_method_id("Iterator", method).expect("Iterator adapter method id")
    );
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Iterator);
    assert_eq!(standard_method.target, target);
    assert_eq!(caches.dynamic_set_count_for(site), 1);

    let actual =
        run_linked_test_entry_with_caches(&Vm::new(), &linked, "main", &[], &mut budget, &caches);
    assert_eq!(actual, Ok(expected));
    assert_eq!(caches.dynamic_get_count_for(site), 2);
    assert_eq!(caches.dynamic_set_count_for(site), 1);
}
