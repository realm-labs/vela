use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use vela_bytecode::compiler::compile_program_source_with_options;
use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId};
use vela_engine::args::{FromScriptArg, IntoScriptArg, ScriptArgsExt};
use vela_engine::engine::Engine;
use vela_engine::runtime::CallOptions;
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;
use vela_host::path::{HostPath, HostRef};
use vela_host::proxy::PathProxy;
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_reflect::registry::{MethodDesc, TypeDesc, TypeKey};
use vela_vm::error::{VmError, VmErrorKind};
use vela_vm::owned_value::OwnedValue;

#[test]
fn script_arg_conversions_support_optional_values() {
    let some_value = OwnedValue::Enum {
        enum_name: "Option".to_owned(),
        variant: "Some".to_owned(),
        fields: [("0".to_owned(), OwnedValue::Int(3))].into(),
    };
    let none_value = OwnedValue::Enum {
        enum_name: "game::std::Option".to_owned(),
        variant: "None".to_owned(),
        fields: [].into(),
    };

    assert_eq!(Option::<i64>::from_script_arg(&OwnedValue::Null), Ok(None));
    assert_eq!(
        Option::<i64>::from_script_arg(&OwnedValue::Int(3)),
        Ok(Some(3))
    );
    assert_eq!(Option::<i64>::from_script_arg(&some_value), Ok(Some(3)));
    assert_eq!(Option::<i64>::from_script_arg(&none_value), Ok(None));
    assert_eq!(
        Some("reward").into_script_arg(),
        OwnedValue::String("reward".to_owned())
    );
    assert_eq!(Option::<i64>::None.into_script_arg(), OwnedValue::Null);
    assert_eq!(
        vela_engine::args![Some(2_i64), Option::<i64>::None],
        vec![OwnedValue::Int(2), OwnedValue::Null],
    );
    assert!(matches!(
        Option::<i64>::from_script_arg(&OwnedValue::String("bad".to_owned())),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "int" },
            ..
        })
    ));
    assert!(matches!(
        Option::<i64>::from_script_arg(&OwnedValue::Enum {
            enum_name: "Option".to_owned(),
            variant: "Missing".to_owned(),
            fields: [].into(),
        }),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch {
                operation: "option"
            },
            ..
        })
    ));
}

#[test]
fn script_arg_conversions_support_result_values() {
    let ok_value = std::result::Result::<i64, String>::Ok(4).into_script_arg();
    let err_value = std::result::Result::<i64, String>::Err("bad".to_owned()).into_script_arg();

    assert_eq!(
        std::result::Result::<i64, String>::from_script_arg(&ok_value),
        Ok(Ok(4)),
    );
    assert_eq!(
        std::result::Result::<i64, String>::from_script_arg(&err_value),
        Ok(Err("bad".to_owned())),
    );
    assert_eq!(
        vela_engine::args![std::result::Result::<i64, String>::Err(
            "missing".to_owned()
        )],
        vec![OwnedValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Err".to_owned(),
            fields: [("0".to_owned(), OwnedValue::String("missing".to_owned()))].into(),
        }],
    );
    assert!(matches!(
        std::result::Result::<i64, String>::from_script_arg(&OwnedValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Ok".to_owned(),
            fields: [("0".to_owned(), OwnedValue::String("bad".to_owned()))].into(),
        }),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "int" },
            ..
        })
    ));
    assert!(matches!(
        std::result::Result::<i64, String>::from_script_arg(&OwnedValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Unknown".to_owned(),
            fields: [("0".to_owned(), OwnedValue::Int(1))].into(),
        }),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch {
                operation: "result",
            },
            ..
        })
    ));
}

#[test]
fn script_arg_conversions_support_set_values() {
    let mut tree = BTreeSet::new();
    tree.insert("fire".to_owned());
    tree.insert("ice".to_owned());
    assert_eq!(
        tree.clone().into_script_arg(),
        OwnedValue::Set(vec![
            OwnedValue::String("fire".to_owned()),
            OwnedValue::String("ice".to_owned()),
        ]),
    );
    assert_eq!(
        BTreeSet::<String>::from_script_arg(&OwnedValue::Set(vec![
            OwnedValue::String("ice".to_owned()),
            OwnedValue::String("fire".to_owned()),
            OwnedValue::String("fire".to_owned()),
        ])),
        Ok(tree),
    );

    let mut hash = HashSet::new();
    hash.insert(2_i64);
    hash.insert(1_i64);
    assert_eq!(
        hash.clone().into_script_arg(),
        OwnedValue::Set(vec![OwnedValue::Int(1), OwnedValue::Int(2)]),
    );
    assert_eq!(
        HashSet::<i64>::from_script_arg(&OwnedValue::Set(vec![
            OwnedValue::Int(1),
            OwnedValue::Int(2),
            OwnedValue::Int(2),
        ])),
        Ok(hash),
    );
    assert!(matches!(
        BTreeSet::<i64>::from_script_arg(&OwnedValue::Array(vec![OwnedValue::Int(1)])),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "set" },
            ..
        })
    ));
}

#[test]
fn args_macro_converts_rust_values_and_host_refs() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 7);
    let proxy = PathProxy::new(HostPath::new(host_ref).field(FieldId::new(9)));
    let mut map = BTreeMap::new();
    map.insert("key", 9);
    let mut hash_map = HashMap::new();
    hash_map.insert("hash", 11);

    let args = vela_engine::args![
        (),
        true,
        5,
        2.5_f64,
        "title",
        ["a", "b"],
        map,
        hash_map,
        host_ref,
        proxy.clone(),
    ];

    assert_eq!(
        args,
        vec![
            OwnedValue::Null,
            OwnedValue::Bool(true),
            OwnedValue::Int(5),
            OwnedValue::Float(2.5),
            OwnedValue::String("title".to_owned()),
            OwnedValue::Array(vec![
                OwnedValue::String("a".to_owned()),
                OwnedValue::String("b".to_owned())
            ]),
            OwnedValue::Map([("key".to_owned(), OwnedValue::Int(9))].into()),
            OwnedValue::Map([("hash".to_owned(), OwnedValue::Int(11))].into()),
            OwnedValue::HostRef(host_ref),
            OwnedValue::PathProxy(proxy),
        ]
    );
    assert_eq!(vela_engine::args!(), Vec::<OwnedValue>::new());
    assert_eq!(vela_engine::host!(1, 42, 7), OwnedValue::HostRef(host_ref));
    assert_eq!(vela_engine::host!(host_ref), OwnedValue::HostRef(host_ref));
    assert_eq!(
        vela_engine::args::host(host_ref),
        OwnedValue::HostRef(host_ref)
    );
    assert_eq!(
        vela_engine::args::host((1_u32, 42_u64, 7_u32)),
        OwnedValue::HostRef(host_ref)
    );
    assert_eq!(
        vela_engine::args::host((HostTypeId::new(1), HostObjectId::new(42), 7_u32)),
        OwnedValue::HostRef(host_ref)
    );
}

#[test]
fn script_arg_conversions_extract_owned_rust_values() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 7);
    let proxy = PathProxy::new(HostPath::new(host_ref).field(FieldId::new(9)));
    let args = vela_engine::args![
        true,
        5,
        2.5_f64,
        "title",
        [1, 2, 3],
        BTreeMap::from([("key", "value")]),
        HashMap::from([("hash", "map")]),
        host_ref,
        proxy.clone(),
    ];

    assert!(args.required::<bool>(0).expect("bool arg"));
    assert_eq!(args.required::<i64>(1), Ok(5));
    assert_eq!(args.required::<f64>(2), Ok(2.5));
    assert_eq!(args.required::<f32>(2), Ok(2.5_f32));
    assert_eq!(args.required::<String>(3), Ok("title".to_owned()));
    assert_eq!(args.required::<Vec<i64>>(4), Ok(vec![1, 2, 3]));
    assert_eq!(args.required::<[i64; 3]>(4), Ok([1, 2, 3]));
    assert_eq!(
        args.required::<BTreeMap<String, String>>(5),
        Ok(BTreeMap::from([("key".to_owned(), "value".to_owned())]))
    );
    assert_eq!(
        args.required::<HashMap<String, String>>(6),
        Ok(HashMap::from([("hash".to_owned(), "map".to_owned())]))
    );
    assert_eq!(args.required::<HostRef>(7), Ok(host_ref));
    assert_eq!(args.required::<PathProxy>(8), Ok(proxy));

    assert!(matches!(
        args.required::<HostRef>(1),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch {
                operation: "host ref"
            },
            source_span: None,
            ..
        })
    ));
    assert!(matches!(
        f32::from_script_arg(&OwnedValue::Float(f64::MAX)),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "float" },
            source_span: None,
            ..
        })
    ));
    assert!(matches!(
        args.required::<[i64; 2]>(4),
        Err(VmError {
            kind: VmErrorKind::TypeMismatch { operation: "array" },
            source_span: None,
            ..
        })
    ));
    assert!(matches!(
        args.required::<i64>(9),
        Err(VmError {
            kind: VmErrorKind::ArityMismatch {
                name,
                expected: 10,
                actual: 9,
            },
            source_span: None,
            ..
        }) if name == "native argument conversion"
    ));
}

#[test]
fn runtime_call_accepts_args_and_host_macros() {
    let method = HostMethodId::new(23);
    let engine = Engine::builder()
        .register_type(
            TypeDesc::new(TypeKey::new(TypeId::new(1), "Player"))
                .host_type(HostTypeId::new(1))
                .method(MethodDesc::new(method, "grant_exp")),
        )
        .build()
        .expect("engine should build");
    let program = compile_program_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player, amount: int) {
    player.grant_exp(amount);
    return amount;
}
"#,
        &engine.compiler_options(),
    )
    .expect("program should compile");
    let mut runtime = vela_engine::runtime::Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = PatchTx::new();
    let args = vela_engine::args![vela_engine::host!(1, 42, 1), 12];

    let result = runtime
        .call_raw(
            "main",
            &args,
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call should run");

    assert_eq!(result, OwnedValue::Int(12));
    assert_eq!(tx.patches().len(), 1);
    assert_eq!(
        tx.patches()[0].op,
        PatchOp::CallHostMethod {
            method,
            args: vec![HostValue::Int(12)]
        }
    );
}
