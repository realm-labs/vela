//! Controlled reflection metadata and value access.

use std::collections::BTreeMap;
use std::fmt;

use vela_common::{FieldId, HostMethodId, HostTypeId, TypeId};
use vela_host::{HostPath, HostRef, HostValue, PatchTx, ScriptStateAdapter};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeKey {
    pub id: TypeId,
    pub name: String,
}

impl TypeKey {
    #[must_use]
    pub fn new(id: TypeId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AttrMap {
    attrs: BTreeMap<String, String>,
}

impl AttrMap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDesc {
    pub key: TypeKey,
    pub host_type_id: Option<HostTypeId>,
    pub fields: Vec<FieldDesc>,
    pub methods: Vec<MethodDesc>,
    pub traits: Vec<TraitDesc>,
    pub variants: Vec<VariantDesc>,
    pub attrs: AttrMap,
}

impl TypeDesc {
    #[must_use]
    pub fn new(key: TypeKey) -> Self {
        Self {
            key,
            host_type_id: None,
            fields: Vec::new(),
            methods: Vec::new(),
            traits: Vec::new(),
            variants: Vec::new(),
            attrs: AttrMap::new(),
        }
    }

    #[must_use]
    pub fn host_type(mut self, host_type_id: HostTypeId) -> Self {
        self.host_type_id = Some(host_type_id);
        self
    }

    #[must_use]
    pub fn field(mut self, field: FieldDesc) -> Self {
        self.fields.push(field);
        self
    }

    #[must_use]
    pub fn method(mut self, method: MethodDesc) -> Self {
        self.methods.push(method);
        self
    }

    #[must_use]
    pub fn trait_impl(mut self, trait_desc: TraitDesc) -> Self {
        self.traits.push(trait_desc);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDesc {
    pub id: FieldId,
    pub name: String,
    pub writable: bool,
    pub attrs: AttrMap,
}

impl FieldDesc {
    #[must_use]
    pub fn new(id: FieldId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            writable: false,
            attrs: AttrMap::new(),
        }
    }

    #[must_use]
    pub fn writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodDesc {
    pub id: HostMethodId,
    pub name: String,
    pub attrs: AttrMap,
}

impl MethodDesc {
    #[must_use]
    pub fn new(id: HostMethodId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            attrs: AttrMap::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitDesc {
    pub name: String,
    pub attrs: AttrMap,
}

impl TraitDesc {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attrs: AttrMap::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDesc {
    pub name: String,
    pub attrs: AttrMap,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeRegistry {
    types_by_key: BTreeMap<TypeKey, TypeDesc>,
    host_keys: BTreeMap<HostTypeId, TypeKey>,
}

impl TypeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, desc: TypeDesc) {
        if let Some(host_type_id) = desc.host_type_id {
            self.host_keys.insert(host_type_id, desc.key.clone());
        }
        self.types_by_key.insert(desc.key.clone(), desc);
    }

    #[must_use]
    pub fn type_of_host(&self, host_ref: HostRef) -> Option<&TypeDesc> {
        let key = self.host_keys.get(&host_ref.type_id)?;
        self.types_by_key.get(key)
    }

    #[must_use]
    pub fn fields(&self, key: &TypeKey) -> Option<&[FieldDesc]> {
        self.types_by_key
            .get(key)
            .map(|desc| desc.fields.as_slice())
    }

    fn host_field(&self, host_ref: HostRef, field_name: &str) -> ReflectResult<&FieldDesc> {
        let desc = self.type_of_host(host_ref).ok_or_else(|| {
            ReflectError::new(ReflectErrorKind::UnknownType {
                host_type_id: host_ref.type_id,
            })
        })?;
        find_field(desc, field_name)
    }

    fn host_method(&self, host_ref: HostRef, method_name: &str) -> ReflectResult<&MethodDesc> {
        let desc = self.type_of_host(host_ref).ok_or_else(|| {
            ReflectError::new(ReflectErrorKind::UnknownType {
                host_type_id: host_ref.type_id,
            })
        })?;
        find_method(desc, method_name)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReflectValue {
    Host(HostValue),
    HostRef(HostRef),
    Record(BTreeMap<String, ReflectValue>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectError {
    pub kind: ReflectErrorKind,
}

impl ReflectError {
    fn new(kind: ReflectErrorKind) -> Self {
        Self { kind }
    }
}

impl fmt::Display for ReflectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for ReflectError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReflectErrorKind {
    UnknownType {
        host_type_id: HostTypeId,
    },
    UnknownField {
        type_name: String,
        field: String,
        candidates: Vec<String>,
    },
    UnknownMethod {
        type_name: String,
        method: String,
        candidates: Vec<String>,
    },
    FieldNotWritable {
        type_name: String,
        field: String,
    },
    InvalidTarget,
    InvalidValue,
    Host(String),
}

pub type ReflectResult<T> = Result<T, ReflectError>;

pub struct ReflectContext<'a> {
    pub registry: &'a TypeRegistry,
    pub adapter: &'a dyn ScriptStateAdapter,
    pub tx: &'a mut PatchTx,
}

pub fn type_of<'a>(registry: &'a TypeRegistry, value: &ReflectValue) -> Option<&'a TypeDesc> {
    match value {
        ReflectValue::HostRef(host_ref) => registry.type_of_host(*host_ref),
        ReflectValue::Host(_) | ReflectValue::Record(_) => None,
    }
}

pub fn fields<'a>(registry: &'a TypeRegistry, key: &TypeKey) -> Option<&'a [FieldDesc]> {
    registry.fields(key)
}

pub fn get(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
) -> ReflectResult<ReflectValue> {
    match target {
        ReflectValue::HostRef(host_ref) => {
            let field_desc = ctx.registry.host_field(*host_ref, field)?;
            let value = ctx
                .tx
                .read_path(ctx.adapter, &HostPath::new(*host_ref).field(field_desc.id))
                .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
            Ok(ReflectValue::Host(value))
        }
        ReflectValue::Record(record) => record
            .get(field)
            .cloned()
            .ok_or_else(|| ReflectError::new(record_unknown_field(field, record))),
        ReflectValue::Host(_) => Err(ReflectError::new(ReflectErrorKind::InvalidTarget)),
    }
}

pub fn set(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    field: &str,
    value: ReflectValue,
) -> ReflectResult<()> {
    match target {
        ReflectValue::HostRef(host_ref) => {
            let field_desc = ctx.registry.host_field(*host_ref, field)?;
            if !field_desc.writable {
                let type_name = ctx
                    .registry
                    .type_of_host(*host_ref)
                    .map_or_else(|| "<unknown>".to_owned(), |desc| desc.key.name.clone());
                return Err(ReflectError::new(ReflectErrorKind::FieldNotWritable {
                    type_name,
                    field: field.to_owned(),
                }));
            }
            let ReflectValue::Host(value) = value else {
                return Err(ReflectError::new(ReflectErrorKind::InvalidValue));
            };
            ctx.tx
                .set_path(HostPath::new(*host_ref).field(field_desc.id), value, None)
                .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
            Ok(())
        }
        ReflectValue::Record(_) | ReflectValue::Host(_) => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
    }
}

pub fn call(
    ctx: &mut ReflectContext<'_>,
    target: &ReflectValue,
    method: &str,
    args: Vec<ReflectValue>,
) -> ReflectResult<ReflectValue> {
    let ReflectValue::HostRef(host_ref) = target else {
        return Err(ReflectError::new(ReflectErrorKind::InvalidTarget));
    };
    let method_desc = ctx.registry.host_method(*host_ref, method)?;
    let args = args
        .into_iter()
        .map(host_arg)
        .collect::<ReflectResult<Vec<_>>>()?;
    ctx.tx
        .call_method(HostPath::new(*host_ref), method_desc.id, args, None)
        .map_err(|error| ReflectError::new(ReflectErrorKind::Host(error.to_string())))?;
    Ok(ReflectValue::Host(HostValue::Null))
}

pub fn implements(
    registry: &TypeRegistry,
    target: &ReflectValue,
    trait_name: &str,
) -> ReflectResult<bool> {
    match target {
        ReflectValue::HostRef(host_ref) => {
            let desc = registry.type_of_host(*host_ref).ok_or_else(|| {
                ReflectError::new(ReflectErrorKind::UnknownType {
                    host_type_id: host_ref.type_id,
                })
            })?;
            Ok(desc
                .traits
                .iter()
                .any(|trait_desc| trait_desc.name == trait_name))
        }
        ReflectValue::Host(_) | ReflectValue::Record(_) => {
            Err(ReflectError::new(ReflectErrorKind::InvalidTarget))
        }
    }
}

fn find_field<'a>(desc: &'a TypeDesc, field: &str) -> ReflectResult<&'a FieldDesc> {
    desc.fields
        .iter()
        .find(|candidate| candidate.name == field)
        .ok_or_else(|| {
            ReflectError::new(ReflectErrorKind::UnknownField {
                type_name: desc.key.name.clone(),
                field: field.to_owned(),
                candidates: name_candidates(
                    field,
                    desc.fields.iter().map(|field| field.name.as_str()),
                ),
            })
        })
}

fn find_method<'a>(desc: &'a TypeDesc, method: &str) -> ReflectResult<&'a MethodDesc> {
    desc.methods
        .iter()
        .find(|candidate| candidate.name == method)
        .ok_or_else(|| {
            ReflectError::new(ReflectErrorKind::UnknownMethod {
                type_name: desc.key.name.clone(),
                method: method.to_owned(),
                candidates: name_candidates(
                    method,
                    desc.methods.iter().map(|method| method.name.as_str()),
                ),
            })
        })
}

fn host_arg(value: ReflectValue) -> ReflectResult<HostValue> {
    let ReflectValue::Host(value) = value else {
        return Err(ReflectError::new(ReflectErrorKind::InvalidValue));
    };
    Ok(value)
}

fn record_unknown_field(field: &str, record: &BTreeMap<String, ReflectValue>) -> ReflectErrorKind {
    ReflectErrorKind::UnknownField {
        type_name: "record".to_owned(),
        field: field.to_owned(),
        candidates: name_candidates(field, record.keys().map(String::as_str)),
    }
}

fn name_candidates<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut candidates = candidates
        .map(|candidate| (edit_distance(name, candidate), candidate.to_owned()))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    candidates
        .into_iter()
        .take(3)
        .map(|(_, candidate)| candidate)
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_ch) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_ch) in right.iter().enumerate() {
            let substitution = usize::from(left_ch != right_ch);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::{HostObjectId, SourceId, Span};
    use vela_host::{HostObjectSnapshot, MockStateAdapter, PatchOp};

    fn player_ref() -> HostRef {
        HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
    }

    fn registry() -> TypeRegistry {
        let mut registry = TypeRegistry::new();
        registry.register(
            TypeDesc::new(TypeKey::new(TypeId::new(100), "Player"))
                .host_type(HostTypeId::new(1))
                .field(FieldDesc::new(FieldId::new(1), "id"))
                .field(FieldDesc::new(FieldId::new(2), "level").writable(true))
                .method(MethodDesc::new(HostMethodId::new(5), "grant_exp"))
                .trait_impl(TraitDesc::new("Damageable")),
        );
        registry
    }

    fn adapter_with_level(value: HostValue) -> MockStateAdapter {
        let mut adapter = MockStateAdapter::new();
        adapter.insert_value(HostPath::new(player_ref()).field(FieldId::new(2)), value);
        adapter
    }

    #[test]
    fn reflect_set_host_ref_creates_patch() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        set(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "level",
            ReflectValue::Host(HostValue::Int(10)),
        )
        .expect("reflect set");

        assert_eq!(ctx.tx.patches().len(), 1);
        assert_eq!(ctx.tx.patches()[0].op, PatchOp::Set(HostValue::Int(10)));
    }

    #[test]
    fn reflect_get_host_ref_reads_overlay_before_adapter() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        tx.set_path(
            HostPath::new(player_ref()).field(FieldId::new(2)),
            HostValue::Int(12),
            Some(Span::new(SourceId::new(1), 0, 1)),
        )
        .expect("set overlay");
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let value =
            get(&mut ctx, &ReflectValue::HostRef(player_ref()), "level").expect("reflect get");

        assert_eq!(value, ReflectValue::Host(HostValue::Int(12)));
    }

    #[test]
    fn reflect_get_record_field_reads_value() {
        let registry = TypeRegistry::new();
        let adapter = MockStateAdapter::new();
        let mut tx = PatchTx::new();
        let mut record = BTreeMap::new();
        record.insert("field".to_owned(), ReflectValue::Host(HostValue::Int(42)));
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let value = get(&mut ctx, &ReflectValue::Record(record), "field").expect("record get");

        assert_eq!(value, ReflectValue::Host(HostValue::Int(42)));
    }

    #[test]
    fn reflect_set_read_only_host_field_fails() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = set(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "id",
            ReflectValue::Host(HostValue::Int(10)),
        )
        .expect_err("read-only set");

        assert_eq!(
            error.kind,
            ReflectErrorKind::FieldNotWritable {
                type_name: "Player".to_owned(),
                field: "id".to_owned()
            }
        );
        assert!(ctx.tx.patches().is_empty());
    }

    #[test]
    fn unknown_fields_include_candidate_hints() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = get(&mut ctx, &ReflectValue::HostRef(player_ref()), "levle")
            .expect_err("unknown field");

        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownField {
                type_name: "Player".to_owned(),
                field: "levle".to_owned(),
                candidates: vec!["level".to_owned(), "id".to_owned()]
            }
        );
    }

    #[test]
    fn type_registry_exposes_host_type_fields() {
        let registry = registry();
        let desc = type_of(&registry, &ReflectValue::HostRef(player_ref())).expect("type desc");
        let fields = fields(&registry, &desc.key).expect("fields");

        assert_eq!(desc.key.name, "Player");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].name, "level");
    }

    #[test]
    fn reflect_get_propagates_host_generation_errors() {
        let registry = registry();
        let mut adapter = MockStateAdapter::new();
        let fresh_ref = player_ref();
        adapter.insert_value(
            HostPath::new(fresh_ref).field(FieldId::new(2)),
            HostValue::Int(9),
        );
        let stale_ref = HostRef::new(fresh_ref.type_id, fresh_ref.object_id, 2);
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error =
            get(&mut ctx, &ReflectValue::HostRef(stale_ref), "level").expect_err("stale get");

        assert!(matches!(error.kind, ReflectErrorKind::Host(_)));
        assert_eq!(
            vela_host::PatchTx::require_fresh_ref(
                stale_ref,
                &HostObjectSnapshot {
                    type_id: fresh_ref.type_id,
                    object_id: fresh_ref.object_id,
                    generation: 3,
                }
            )
            .expect_err("stale ref")
            .kind,
            vela_host::HostErrorKind::StaleGeneration {
                expected: 2,
                actual: 3
            }
        );
    }

    #[test]
    fn reflect_call_host_ref_records_patch() {
        let registry = registry();
        let mut adapter = adapter_with_level(HostValue::Int(9));
        adapter.insert_method_return(HostMethodId::new(5), HostValue::Null);
        let mut tx = PatchTx::new();
        {
            let mut ctx = ReflectContext {
                registry: &registry,
                adapter: &adapter,
                tx: &mut tx,
            };

            let value = call(
                &mut ctx,
                &ReflectValue::HostRef(player_ref()),
                "grant_exp",
                vec![ReflectValue::Host(HostValue::Int(20))],
            )
            .expect("reflect call");

            assert_eq!(value, ReflectValue::Host(HostValue::Null));
            assert_eq!(ctx.tx.patches().len(), 1);
            assert_eq!(
                ctx.tx.patches()[0].op,
                PatchOp::CallHostMethod {
                    method: HostMethodId::new(5),
                    args: vec![HostValue::Int(20)]
                }
            );
            assert!(adapter.method_calls().is_empty());
        }

        tx.apply(&mut adapter).expect("apply reflect call");
        assert_eq!(
            adapter.method_calls(),
            &[(
                HostPath::new(player_ref()),
                HostMethodId::new(5),
                vec![HostValue::Int(20)]
            )]
        );
    }

    #[test]
    fn reflect_call_rejects_non_host_args() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = call(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "grant_exp",
            vec![ReflectValue::Record(BTreeMap::new())],
        )
        .expect_err("invalid arg");

        assert_eq!(error.kind, ReflectErrorKind::InvalidValue);
        assert!(ctx.tx.patches().is_empty());
    }

    #[test]
    fn unknown_methods_include_candidate_hints() {
        let registry = registry();
        let adapter = adapter_with_level(HostValue::Int(9));
        let mut tx = PatchTx::new();
        let mut ctx = ReflectContext {
            registry: &registry,
            adapter: &adapter,
            tx: &mut tx,
        };

        let error = call(
            &mut ctx,
            &ReflectValue::HostRef(player_ref()),
            "grant_xp",
            Vec::new(),
        )
        .expect_err("unknown method");

        assert_eq!(
            error.kind,
            ReflectErrorKind::UnknownMethod {
                type_name: "Player".to_owned(),
                method: "grant_xp".to_owned(),
                candidates: vec!["grant_exp".to_owned()]
            }
        );
    }

    #[test]
    fn reflect_implements_uses_registry_metadata() {
        let registry = registry();

        assert!(
            implements(
                &registry,
                &ReflectValue::HostRef(player_ref()),
                "Damageable"
            )
            .expect("implements check")
        );
        assert!(
            !implements(&registry, &ReflectValue::HostRef(player_ref()), "Inventory")
                .expect("implements check")
        );
    }
}
