use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use vela_common::{HostMethodId, HostObjectId, HostTypeId};
use vela_def::{FieldId, TypeId};
use vela_engine::args::{FromScriptArg, IntoScriptArg, ScriptArgsExt};
use vela_engine::engine::Engine;
use vela_engine::runtime::CallOptions;
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::proxy::PathProxy;
use vela_reflect::registry::{MethodDesc, TypeDesc, TypeKey};
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

#[test]
fn script_arg_conversions_preserve_exact_scalar_tags() {
    assert_eq!(
        1_i8.into_script_arg(),
        OwnedValue::Scalar(vela_common::ScalarValue::I8(1))
    );
    assert_eq!(
        2_i16.into_script_arg(),
        OwnedValue::Scalar(vela_common::ScalarValue::I16(2))
    );
    assert_eq!(
        3_i32.into_script_arg(),
        OwnedValue::Scalar(vela_common::ScalarValue::I32(3))
    );
    assert_eq!(
        4_i64.into_script_arg(),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(4))
    );
    assert_eq!(
        5_u8.into_script_arg(),
        OwnedValue::Scalar(vela_common::ScalarValue::U8(5))
    );
    assert_eq!(
        6_u16.into_script_arg(),
        OwnedValue::Scalar(vela_common::ScalarValue::U16(6))
    );
    assert_eq!(
        7_u32.into_script_arg(),
        OwnedValue::Scalar(vela_common::ScalarValue::U32(7))
    );
    assert_eq!(
        8_u64.into_script_arg(),
        OwnedValue::Scalar(vela_common::ScalarValue::U64(8))
    );
    assert_eq!(
        1.5_f32.into_script_arg(),
        OwnedValue::Scalar(vela_common::ScalarValue::F32(1.5))
    );
    assert_eq!(
        2.5_f64.into_script_arg(),
        OwnedValue::Scalar(vela_common::ScalarValue::F64(2.5))
    );
    assert_eq!('奖'.into_script_arg(), OwnedValue::Char('奖'));

    assert_eq!(
        u64::from_script_arg(&OwnedValue::Scalar(vela_common::ScalarValue::U64(9))),
        Ok(9)
    );
    assert_eq!(char::from_script_arg(&OwnedValue::Char('奖')), Ok('奖'));
    assert!(matches!(
        i64::from_script_arg(&OwnedValue::Scalar(vela_common::ScalarValue::I32(9))),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "i64" })
    ));
    assert!(matches!(
        char::from_script_arg(&OwnedValue::String("奖".to_owned())),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "char" })
    ));
}

#[test]
fn script_arg_conversions_round_trip_byte_buffers_as_bytes() {
    assert_eq!(
        vec![0_u8, 1, 255].into_script_arg(),
        OwnedValue::Bytes(vec![0, 1, 255])
    );
    assert_eq!(
        (&[2_u8, 3, 4][..]).into_script_arg(),
        OwnedValue::Bytes(vec![2, 3, 4])
    );
    assert_eq!(
        Vec::<u8>::from_script_arg(&OwnedValue::Bytes(vec![5, 6, 7])),
        Ok(vec![5, 6, 7])
    );
    assert!(matches!(
        Vec::<u8>::from_script_arg(&OwnedValue::Array(vec![OwnedValue::Scalar(
            vela_common::ScalarValue::U8(1)
        )])),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "bytes" })
    ));
    assert_eq!(
        vec![1_i64, 2_i64].into_script_arg(),
        OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        ])
    );
}

#[test]
fn script_arg_conversions_support_optional_values() {
    let some_value = OwnedValue::Enum {
        enum_name: "Option".to_owned(),
        variant: "Some".to_owned(),
        fields: [(
            "0".to_owned(),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(3)),
        )]
        .into(),
    };
    let none_value = OwnedValue::Enum {
        enum_name: "game::std::Option".to_owned(),
        variant: "None".to_owned(),
        fields: [].into(),
    };

    assert_eq!(Option::<i64>::from_script_arg(&OwnedValue::Null), Ok(None));
    assert_eq!(
        Option::<i64>::from_script_arg(&OwnedValue::Scalar(vela_common::ScalarValue::I64(3))),
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
        vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
            OwnedValue::Null
        ],
    );
    assert!(matches!(
        Option::<i64>::from_script_arg(&OwnedValue::String("bad".to_owned())),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "i64" })
    ));
    assert!(matches!(
        Option::<i64>::from_script_arg(&OwnedValue::Enum {
            enum_name: "Option".to_owned(),
            variant: "Missing".to_owned(),
            fields: [].into(),
        }),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "option" })
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
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "i64" })
    ));
    assert!(matches!(
        std::result::Result::<i64, String>::from_script_arg(&OwnedValue::Enum {
            enum_name: "Result".to_owned(),
            variant: "Unknown".to_owned(),
            fields: [("0".to_owned(), OwnedValue::Scalar(vela_common::ScalarValue::I64(1)))].into(),
        }),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "result" })
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
        OwnedValue::Set(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2))
        ]),
    );
    assert_eq!(
        HashSet::<i64>::from_script_arg(&OwnedValue::Set(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(1)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        ])),
        Ok(hash),
    );
    assert!(matches!(
        BTreeSet::<i64>::from_script_arg(&OwnedValue::Array(vec![OwnedValue::Scalar(vela_common::ScalarValue::I64(1))])),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "set" })
    ));
}

#[test]
fn script_arg_conversions_support_non_string_map_keys() {
    let ordered = BTreeMap::from([(1_i64, "one"), (2_i64, "two")]);
    assert_eq!(
        ordered.clone().into_script_arg(),
        OwnedValue::map([(1_i64, "one"), (2_i64, "two")])
    );
    assert_eq!(
        BTreeMap::<i64, String>::from_script_arg(&OwnedValue::map([
            (1_i64, "one"),
            (2_i64, "two"),
        ])),
        Ok(BTreeMap::from([
            (1_i64, "one".to_owned()),
            (2_i64, "two".to_owned()),
        ]))
    );

    let hash = HashMap::from([(1_i64, "one"), (2_i64, "two")]);
    assert_eq!(
        HashMap::<i64, String>::from_script_arg(&hash.clone().into_script_arg()),
        Ok(HashMap::from([
            (1_i64, "one".to_owned()),
            (2_i64, "two".to_owned()),
        ]))
    );
    assert!(matches!(
        BTreeMap::<i64, String>::from_script_arg(&OwnedValue::map([("not-int", "bad")])),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "i64" })
    ));
}

#[test]
fn args_macro_converts_rust_values_and_host_refs() {
    let host_ref = HostRef::new(HostTypeId::new(1), HostObjectId::new(42), 7);
    let proxy = PathProxy::from_diagnostic_path(HostPath::new(host_ref).field(FieldId::new(9)));
    let mut map = BTreeMap::new();
    map.insert("key", 9_i64);
    let mut hash_map = HashMap::new();
    hash_map.insert("hash", 11_i64);

    let args = vela_engine::args![
        (),
        true,
        5_i64,
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
            OwnedValue::Scalar(vela_common::ScalarValue::I64(5)),
            OwnedValue::Scalar(vela_common::ScalarValue::F64(2.5)),
            OwnedValue::String("title".to_owned()),
            OwnedValue::Array(vec![
                OwnedValue::String("a".to_owned()),
                OwnedValue::String("b".to_owned())
            ]),
            OwnedValue::map([("key", OwnedValue::Scalar(vela_common::ScalarValue::I64(9)))]),
            OwnedValue::map([(
                "hash",
                OwnedValue::Scalar(vela_common::ScalarValue::I64(11))
            )]),
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
    let proxy = PathProxy::from_diagnostic_path(HostPath::new(host_ref).field(FieldId::new(9)));
    let args = vela_engine::args![
        true,
        5_i64,
        2.5_f64,
        "title",
        [1_i64, 2_i64, 3_i64],
        BTreeMap::from([("key", "value")]),
        HashMap::from([("hash", "map")]),
        host_ref,
        proxy.clone(),
    ];

    assert!(args.required::<bool>(0).expect("bool arg"));
    assert_eq!(args.required::<i64>(1), Ok(5));
    assert_eq!(args.required::<f64>(2), Ok(2.5));
    assert_eq!(
        f32::from_script_arg(&OwnedValue::Scalar(vela_common::ScalarValue::F32(2.5))),
        Ok(2.5_f32)
    );
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
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "host ref" })
            && error.source_span.is_none()
    ));
    assert!(matches!(
        f32::from_script_arg(&OwnedValue::Scalar(vela_common::ScalarValue::F64(f64::MAX))),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "f32" })
            && error.source_span.is_none()
    ));
    assert!(matches!(
        args.required::<[i64; 2]>(4),
        Err(error) if matches!(error.kind(), VmErrorKind::TypeMismatch { operation: "array" })
            && error.source_span.is_none()
    ));
    assert!(matches!(
        args.required::<i64>(9),
        Err(error) if matches!(error.kind(), VmErrorKind::ArityMismatch {
                name,
                expected: 10,
                actual: 9,
            } if name == "native argument conversion")
            && error.source_span.is_none()
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
    let program = engine
        .compile_source(
            r#"
fn main(player: Player, amount: i64) {
    player.grant_exp(amount);
    return amount;
}
"#,
        )
        .expect("program should compile");
    let mut runtime = vela_engine::runtime::Runtime::new(engine, program);
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let args = vela_engine::args![vela_engine::host!(1, 42, 1), 12_i64];

    let result = runtime
        .call_raw(
            "main",
            &args,
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx,
        )
        .expect("runtime call should run");

    assert_eq!(
        result,
        OwnedValue::Scalar(vela_common::ScalarValue::I64(12))
    );
}
