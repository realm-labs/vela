use std::collections::{BTreeMap, BTreeSet, HashMap};

use vela_common::PrimitiveTag;
use vela_hir::ids::HirLocalId;

use super::record_reflection_shapes;
use super::value_types::{RuntimeTypeFact, StandardRuntimeType};

mod queries;
mod syntax_shapes;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ValueShapeFlow {
    locals: HashMap<HirLocalId, ValueShape>,
    names: HashMap<String, ValueShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ValueShape {
    Unknown,
    Scalar(String),
    Record(RecordShape),
    Array(Box<ValueShape>),
    Iterator(Box<ValueShape>),
    Map {
        key: Box<ValueShape>,
        value: Box<ValueShape>,
    },
    Set(Box<ValueShape>),
    Tuple(Vec<ValueShape>),
    Option(Box<ValueShape>),
    Result {
        ok: Option<Box<ValueShape>>,
        err: Option<Box<ValueShape>>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RecordShape {
    type_name: Option<String>,
    fields: BTreeMap<String, RecordFieldShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordFieldShape {
    slot: usize,
    value_type: Option<RuntimeTypeFact>,
    value: Option<ValueShape>,
}

impl ValueShapeFlow {
    pub(super) fn local(&self, local: HirLocalId) -> Option<ValueShape> {
        self.locals.get(&local).cloned()
    }

    pub(super) fn name(&self, name: &str) -> Option<ValueShape> {
        self.names.get(name).cloned()
    }

    pub(super) fn set_name(&mut self, name: impl Into<String>, shape: Option<ValueShape>) {
        let name = name.into();
        if let Some(shape) = shape {
            self.names.insert(name, shape);
        } else {
            self.names.remove(&name);
        }
    }

    pub(super) fn set_local(
        &mut self,
        local: HirLocalId,
        name: impl Into<String>,
        shape: Option<ValueShape>,
    ) {
        let name = name.into();
        if let Some(shape) = shape {
            self.locals.insert(local, shape.clone());
            self.names.insert(name, shape);
        } else {
            self.locals.remove(&local);
            self.names.remove(&name);
        }
    }
}

impl ValueShape {
    pub(super) fn as_record(&self) -> Option<&RecordShape> {
        match self {
            Self::Record(shape) => Some(shape),
            Self::Unknown
            | Self::Scalar(_)
            | Self::Array(_)
            | Self::Iterator(_)
            | Self::Map { .. }
            | Self::Set(_)
            | Self::Tuple(_)
            | Self::Option(_)
            | Self::Result { .. } => None,
        }
    }

    pub(super) fn value_type(&self) -> Option<RuntimeTypeFact> {
        match self {
            Self::Unknown => None,
            Self::Scalar(type_name) => scalar_shape_type_fact(type_name),
            Self::Record(_) => None,
            Self::Array(element) => element
                .value_type()
                .map(RuntimeTypeFact::array)
                .or_else(|| Some(RuntimeTypeFact::standard(StandardRuntimeType::Array))),
            Self::Iterator(item) => item
                .value_type()
                .map(RuntimeTypeFact::iterator)
                .or_else(|| Some(RuntimeTypeFact::standard(StandardRuntimeType::Iterator))),
            Self::Map { key, value } => match (key.value_type(), value.value_type()) {
                (Some(key), Some(value)) => Some(RuntimeTypeFact::map(key, value)),
                _ => Some(RuntimeTypeFact::standard(StandardRuntimeType::Map)),
            },
            Self::Set(element) => element
                .value_type()
                .map(RuntimeTypeFact::set)
                .or_else(|| Some(RuntimeTypeFact::standard(StandardRuntimeType::Set))),
            Self::Tuple(elements) => Some(RuntimeTypeFact::tuple(
                elements
                    .iter()
                    .map(ValueShape::value_type)
                    .collect::<Option<Vec<_>>>()?,
            )),
            Self::Option(value) => value
                .value_type()
                .map(|payload| RuntimeTypeFact::Option(Box::new(payload)))
                .or_else(|| Some(RuntimeTypeFact::standard(StandardRuntimeType::Option))),
            Self::Result { ok, err } => match (
                ok.as_deref().and_then(ValueShape::value_type),
                err.as_deref().and_then(ValueShape::value_type),
            ) {
                (Some(ok), Some(err)) => Some(RuntimeTypeFact::Result {
                    ok: Box::new(ok),
                    err: Box::new(err),
                }),
                _ => Some(RuntimeTypeFact::standard(StandardRuntimeType::Result)),
            },
        }
    }

    pub(super) fn from_runtime_type(fact: RuntimeTypeFact) -> Self {
        let type_name = match fact {
            RuntimeTypeFact::Primitive(PrimitiveTag::Unit) => "()",
            RuntimeTypeFact::Primitive(PrimitiveTag::Bool) => "bool",
            RuntimeTypeFact::Primitive(PrimitiveTag::Char) => "char",
            RuntimeTypeFact::Primitive(PrimitiveTag::I8) => "i8",
            RuntimeTypeFact::Primitive(PrimitiveTag::I16) => "i16",
            RuntimeTypeFact::Primitive(PrimitiveTag::I32) => "i32",
            RuntimeTypeFact::Primitive(PrimitiveTag::I64) => "i64",
            RuntimeTypeFact::Primitive(PrimitiveTag::U8) => "u8",
            RuntimeTypeFact::Primitive(PrimitiveTag::U16) => "u16",
            RuntimeTypeFact::Primitive(PrimitiveTag::U32) => "u32",
            RuntimeTypeFact::Primitive(PrimitiveTag::U64) => "u64",
            RuntimeTypeFact::Primitive(PrimitiveTag::F32) => "f32",
            RuntimeTypeFact::Primitive(PrimitiveTag::F64) => "f64",
            RuntimeTypeFact::Primitive(PrimitiveTag::String) => "String",
            RuntimeTypeFact::Primitive(PrimitiveTag::Bytes) => "Bytes",
            RuntimeTypeFact::Standard(StandardRuntimeType::Array) => {
                return Self::Array(Box::new(Self::Unknown));
            }
            RuntimeTypeFact::Array(element) => {
                return Self::Array(Box::new(Self::from_runtime_type(*element)));
            }
            RuntimeTypeFact::Standard(StandardRuntimeType::Map) => {
                return Self::Map {
                    key: Box::new(Self::Unknown),
                    value: Box::new(Self::Unknown),
                };
            }
            RuntimeTypeFact::Map { key, value } => {
                return Self::Map {
                    key: Box::new(Self::from_runtime_type(*key)),
                    value: Box::new(Self::from_runtime_type(*value)),
                };
            }
            RuntimeTypeFact::Standard(StandardRuntimeType::Set) => {
                return Self::Set(Box::new(Self::Unknown));
            }
            RuntimeTypeFact::Set(element) => {
                return Self::Set(Box::new(Self::from_runtime_type(*element)));
            }
            RuntimeTypeFact::Tuple(elements) => {
                return Self::Tuple(
                    elements
                        .into_iter()
                        .map(Self::from_runtime_type)
                        .collect::<Vec<_>>(),
                );
            }
            RuntimeTypeFact::Standard(StandardRuntimeType::Range) => "Range",
            RuntimeTypeFact::Standard(StandardRuntimeType::Function) => "Function",
            RuntimeTypeFact::Standard(StandardRuntimeType::Closure) => "Closure",
            RuntimeTypeFact::Standard(StandardRuntimeType::Iterator) => {
                return Self::Iterator(Box::new(Self::Unknown));
            }
            RuntimeTypeFact::Iterator(item) => {
                return Self::Iterator(Box::new(Self::from_runtime_type(*item)));
            }
            RuntimeTypeFact::Standard(StandardRuntimeType::Option) => {
                return Self::Option(Box::new(Self::Unknown));
            }
            RuntimeTypeFact::Standard(StandardRuntimeType::Result) => {
                return Self::Result {
                    ok: None,
                    err: None,
                };
            }
            RuntimeTypeFact::Option(payload) => {
                return Self::Option(Box::new(Self::from_runtime_type(*payload)));
            }
            RuntimeTypeFact::Result { ok, err } => {
                return Self::Result {
                    ok: Some(Box::new(Self::from_runtime_type(*ok))),
                    err: Some(Box::new(Self::from_runtime_type(*err))),
                };
            }
        };
        Self::Scalar(type_name.to_owned())
    }

    pub(super) fn array_element(&self) -> Option<&ValueShape> {
        match self {
            Self::Array(element) => Some(element),
            _ => None,
        }
    }

    pub(super) fn array_element_record(&self) -> Option<&RecordShape> {
        self.array_element().and_then(ValueShape::as_record)
    }

    fn iterator_item(&self) -> Option<&ValueShape> {
        match self {
            Self::Iterator(item) => Some(item),
            _ => None,
        }
    }

    pub(super) fn map_parts(&self) -> Option<(&ValueShape, &ValueShape)> {
        match self {
            Self::Map { key, value } => Some((key, value)),
            _ => None,
        }
    }

    pub(super) fn map_entry(key: ValueShape, value: ValueShape) -> Self {
        Self::Record(RecordShape::from_field_shapes_with_type(
            Some("MapEntry".to_owned()),
            [("key".to_owned(), key), ("value".to_owned(), value)],
        ))
    }
}

fn scalar_shape_type_fact(type_name: &str) -> Option<RuntimeTypeFact> {
    match type_name {
        "Unit" | "()" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Unit)),
        "Bool" | "bool" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bool)),
        "I8" | "i8" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I8)),
        "I16" | "i16" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I16)),
        "I32" | "i32" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I32)),
        "I64" | "i64" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I64)),
        "U8" | "u8" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U8)),
        "U16" | "u16" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U16)),
        "U32" | "u32" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U32)),
        "U64" | "u64" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U64)),
        "F32" | "f32" => Some(RuntimeTypeFact::primitive(PrimitiveTag::F32)),
        "F64" | "f64" => Some(RuntimeTypeFact::primitive(PrimitiveTag::F64)),
        "String" => Some(RuntimeTypeFact::primitive(PrimitiveTag::String)),
        "Bytes" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bytes)),
        "Range" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Range)),
        "Function" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Function)),
        "Closure" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Closure)),
        "Iterator" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Iterator)),
        _ => None,
    }
}

impl RecordShape {
    pub(super) fn from_field_shapes(
        fields: impl IntoIterator<Item = (String, ValueShape)>,
    ) -> Self {
        Self::from_field_shapes_with_type(None, fields)
    }

    fn from_field_shapes_with_type(
        type_name: Option<String>,
        fields: impl IntoIterator<Item = (String, ValueShape)>,
    ) -> Self {
        let fields = fields.into_iter().collect::<BTreeMap<_, _>>();
        let fields = fields
            .into_iter()
            .enumerate()
            .map(|(slot, (field, value))| {
                (
                    field,
                    RecordFieldShape {
                        slot,
                        value_type: value.value_type(),
                        value: Some(value),
                    },
                )
            })
            .collect();
        Self { type_name, fields }
    }

    pub(super) fn type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
    }

    pub(super) fn field_slot(&self, field: &str) -> Option<usize> {
        self.fields.get(field).map(|shape| shape.slot)
    }

    pub(super) fn field_record_shape(&self, field: &str) -> Option<&RecordShape> {
        self.fields
            .get(field)
            .and_then(|shape| shape.value.as_ref())
            .and_then(ValueShape::as_record)
    }

    pub(super) fn field_value_shape(&self, field: &str) -> Option<&ValueShape> {
        self.fields
            .get(field)
            .and_then(|shape| shape.value.as_ref())
    }
}

fn common_shape(mut shapes: Vec<ValueShape>) -> Option<ValueShape> {
    let first = shapes.pop()?;
    shapes.iter().all(|shape| shape == &first).then_some(first)
}

pub(super) fn callback_param_shapes(
    receiver: &ValueShape,
    method: &str,
    param_count: usize,
) -> Option<Vec<Option<ValueShape>>> {
    match receiver {
        ValueShape::Array(element) => Some(vec![Some((**element).clone())]),
        ValueShape::Set(element) => Some(vec![Some((**element).clone())]),
        ValueShape::Map { key, value } => {
            if param_count <= 1 {
                Some(vec![Some((**value).clone())])
            } else {
                Some(vec![Some((**key).clone()), Some((**value).clone())])
            }
        }
        ValueShape::Option(value) => {
            if method == "or_else" {
                Some(Vec::new())
            } else {
                Some(vec![Some((**value).clone())])
            }
        }
        ValueShape::Result { ok, err } => match method {
            "map" | "and_then" => Some(vec![ok.as_deref().cloned()]),
            "map_err" => Some(vec![err.as_deref().cloned()]),
            _ => None,
        },
        _ => None,
    }
}

impl super::Compiler<'_, '_> {
    pub(super) fn record_shape_for_type(&self, type_name: &str) -> Option<RecordShape> {
        self.record_shape_for_type_inner(type_name, &mut BTreeSet::new())
    }

    pub(in crate::compiler) fn schema_record_field_value_type(
        &self,
        root_type: Option<&str>,
        fields: &[String],
    ) -> Option<RuntimeTypeFact> {
        let mut current_type = root_type?.to_owned();
        let (leaf, parents) = fields.split_last()?;
        for field in parents {
            current_type = self
                .facts
                .script_field_slots
                .record_field_fact(&current_type, field)?
                .type_name;
        }
        self.facts
            .script_field_slots
            .record_field_value_type(&current_type, leaf)
    }

    fn record_shape_for_type_inner(
        &self,
        type_name: &str,
        visiting: &mut BTreeSet<String>,
    ) -> Option<RecordShape> {
        if !visiting.insert(type_name.to_owned()) {
            return None;
        }
        let fields = self
            .facts
            .script_field_slots
            .record_fields(type_name)
            .into_iter()
            .map(|(field, script_fact, value_type)| {
                let value = script_fact
                    .as_ref()
                    .and_then(|fact| {
                        self.record_shape_for_type_inner(&fact.type_name, visiting)
                            .map(ValueShape::Record)
                    })
                    .or_else(|| value_type.map(ValueShape::from_runtime_type))
                    .unwrap_or(ValueShape::Unknown);
                (field, value)
            })
            .collect::<Vec<_>>();
        visiting.remove(type_name);
        (!fields.is_empty())
            .then(|| RecordShape::from_field_shapes_with_type(Some(type_name.to_owned()), fields))
    }
}
