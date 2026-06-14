use std::collections::BTreeMap;
use std::slice;

use ::serde::de::{
    self, DeserializeOwned, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use ::serde::forward_to_deserialize_any;

use super::{Error, Result};
use crate::error::VmResult;
use crate::heap::{HeapValue, ScriptHeap};
use crate::script_object::ScriptFields;
use crate::value::Value;

pub fn from_runtime_value<T>(value: &Value, heap: &ScriptHeap) -> VmResult<T>
where
    T: DeserializeOwned,
{
    ::serde::Deserialize::deserialize(RuntimeValueDeserializer { value, heap }).map_err(Into::into)
}

#[derive(Clone, Copy)]
struct RuntimeValueDeserializer<'de> {
    value: &'de Value,
    heap: &'de ScriptHeap,
}

impl<'de> RuntimeValueDeserializer<'de> {
    fn heap_value(self) -> Result<&'de HeapValue> {
        let Value::HeapRef(reference) = self.value else {
            return Err(Error::custom("expected heap value"));
        };
        self.heap
            .get(*reference)
            .ok_or_else(|| Error::custom("invalid heap reference"))
    }
}

impl<'de> de::Deserializer<'de> for RuntimeValueDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::Missing | Value::Null => visitor.visit_unit(),
            Value::Bool(value) => visitor.visit_bool(*value),
            Value::Char(value) => visitor.visit_char(*value),
            Value::I8(value) => visitor.visit_i8(*value),
            Value::I16(value) => visitor.visit_i16(*value),
            Value::I32(value) => visitor.visit_i32(*value),
            Value::I64(value) => visitor.visit_i64(*value),
            Value::U8(value) => visitor.visit_u8(*value),
            Value::U16(value) => visitor.visit_u16(*value),
            Value::U32(value) => visitor.visit_u32(*value),
            Value::U64(value) => visitor.visit_u64(*value),
            Value::F32(value) => visitor.visit_f32(*value),
            Value::F64(value) => visitor.visit_f64(*value),
            Value::HeapRef(_) => match self.heap_value()? {
                HeapValue::String(value) => visitor.visit_str(value),
                HeapValue::Bytes(value) => visitor.visit_bytes(value),
                HeapValue::Array(values) => visitor.visit_seq(RuntimeSeqAccess {
                    iter: RuntimeSeqIter::Slice(values.iter()),
                    heap: self.heap,
                }),
                HeapValue::Set(values) => visitor.visit_seq(RuntimeSeqAccess {
                    iter: RuntimeSeqIter::Set(values.iter_values()),
                    heap: self.heap,
                }),
                HeapValue::Map(values) => {
                    visitor.visit_map(RuntimeMapAccess::from_map(values, self.heap))
                }
                HeapValue::Record { fields, .. } => {
                    visitor.visit_map(RuntimeMapAccess::from_fields(fields, self.heap))
                }
                HeapValue::Enum { .. } => self.deserialize_enum("", &[], visitor),
                HeapValue::Closure(_) | HeapValue::Iterator(_) | HeapValue::PathProxy(_) => {
                    Err(Error::custom("unsupported runtime serde value"))
                }
            },
            Value::Range(_) | Value::HostRef(_) => {
                Err(Error::custom("unsupported runtime serde value"))
            }
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::Bool(value) => visitor.visit_bool(*value),
            _ => Err(Error::custom("expected bool")),
        }
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::Char(value) => visitor.visit_char(*value),
            _ => Err(Error::custom("expected char")),
        }
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::I64(value) => visitor.visit_i64(*value),
            _ => Err(Error::custom("expected int")),
        }
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::U64(value) => visitor.visit_u64(*value),
            _ => Err(Error::custom("expected unsigned int")),
        }
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::F64(value) => visitor.visit_f64(*value),
            Value::I64(value) => visitor.visit_f64(*value as f64),
            _ => Err(Error::custom("expected float")),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.heap_value()? {
            HeapValue::String(value) => visitor.visit_str(value),
            _ => Err(Error::custom("expected string")),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::Missing | Value::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::Missing | Value::Null => visitor.visit_unit(),
            Value::HeapRef(_) => match self.heap_value()? {
                HeapValue::Record { fields, .. } if fields.is_empty() => visitor.visit_unit(),
                _ => Err(Error::custom("expected unit")),
            },
            _ => Err(Error::custom("expected unit")),
        }
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.heap_value()? {
            HeapValue::Array(values) => visitor.visit_seq(RuntimeSeqAccess {
                iter: RuntimeSeqIter::Slice(values.iter()),
                heap: self.heap,
            }),
            HeapValue::Set(values) => visitor.visit_seq(RuntimeSeqAccess {
                iter: RuntimeSeqIter::Set(values.iter_values()),
                heap: self.heap,
            }),
            _ => Err(Error::custom("expected sequence")),
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.heap_value()? {
            HeapValue::Map(values) => {
                visitor.visit_map(RuntimeMapAccess::from_map(values, self.heap))
            }
            HeapValue::Record { fields, .. } => {
                visitor.visit_map(RuntimeMapAccess::from_fields(fields, self.heap))
            }
            _ => Err(Error::custom("expected map")),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.heap_value()? {
            HeapValue::Record { fields, .. } => {
                visitor.visit_map(RuntimeMapAccess::from_fields(fields, self.heap))
            }
            HeapValue::Map(values) => {
                visitor.visit_map(RuntimeMapAccess::from_map(values, self.heap))
            }
            _ => Err(Error::custom("expected struct")),
        }
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.heap_value()? {
            HeapValue::Enum {
                variant, fields, ..
            } => visitor.visit_enum(RuntimeEnumAccess {
                variant,
                fields,
                heap: self.heap,
            }),
            _ => Err(Error::custom("expected enum")),
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    forward_to_deserialize_any! {
        i8 i16 i32 u8 u16 u32 f32 bytes byte_buf
    }
}

enum RuntimeSeqIter<'de> {
    Slice(slice::Iter<'de, Value>),
    Set(std::collections::btree_map::Values<'de, crate::value_key::ValueKey, Value>),
}

impl<'de> RuntimeSeqIter<'de> {
    fn next(&mut self) -> Option<&'de Value> {
        match self {
            Self::Slice(iter) => iter.next(),
            Self::Set(iter) => iter.next(),
        }
    }
}

struct RuntimeSeqAccess<'de> {
    iter: RuntimeSeqIter<'de>,
    heap: &'de ScriptHeap,
}

impl<'de> SeqAccess<'de> for RuntimeSeqAccess<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        self.iter
            .next()
            .map(|value| {
                seed.deserialize(RuntimeValueDeserializer {
                    value,
                    heap: self.heap,
                })
            })
            .transpose()
    }
}

struct RuntimeMapAccess<'de> {
    entries: Vec<(&'de str, &'de Value)>,
    next_value: Option<&'de Value>,
    heap: &'de ScriptHeap,
}

impl<'de> RuntimeMapAccess<'de> {
    fn from_map(values: &'de BTreeMap<String, Value>, heap: &'de ScriptHeap) -> Self {
        Self {
            entries: values
                .iter()
                .map(|(key, value)| (key.as_str(), value))
                .collect(),
            next_value: None,
            heap,
        }
    }

    fn from_fields(fields: &'de ScriptFields<Value>, heap: &'de ScriptHeap) -> Self {
        Self {
            entries: fields.iter().collect(),
            next_value: None,
            heap,
        }
    }
}

impl<'de> MapAccess<'de> for RuntimeMapAccess<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: de::DeserializeSeed<'de>,
    {
        let Some((key, value)) = self.entries.pop() else {
            return Ok(None);
        };
        self.next_value = Some(value);
        seed.deserialize(key.into_deserializer()).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'de>,
    {
        let value = self
            .next_value
            .take()
            .ok_or_else(|| Error::custom("map value requested before key"))?;
        seed.deserialize(RuntimeValueDeserializer {
            value,
            heap: self.heap,
        })
    }
}

struct RuntimeEnumAccess<'de> {
    variant: &'de str,
    fields: &'de ScriptFields<Value>,
    heap: &'de ScriptHeap,
}

impl<'de> EnumAccess<'de> for RuntimeEnumAccess<'de> {
    type Error = Error;
    type Variant = RuntimeVariantAccess<'de>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: de::DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(self.variant.into_deserializer())?;
        Ok((
            variant,
            RuntimeVariantAccess {
                fields: self.fields,
                heap: self.heap,
            },
        ))
    }
}

struct RuntimeVariantAccess<'de> {
    fields: &'de ScriptFields<Value>,
    heap: &'de ScriptHeap,
}

impl<'de> VariantAccess<'de> for RuntimeVariantAccess<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        if self.fields.is_empty() {
            Ok(())
        } else {
            Err(Error::custom("expected unit variant"))
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: de::DeserializeSeed<'de>,
    {
        let value = self
            .fields
            .get("0")
            .or_else(|| {
                (self.fields.len() == 1)
                    .then(|| self.fields.values().next())
                    .flatten()
            })
            .ok_or_else(|| Error::custom("expected newtype variant payload"))?;
        seed.deserialize(RuntimeValueDeserializer {
            value,
            heap: self.heap,
        })
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(RuntimeTupleVariantSeqAccess {
            fields: self.fields,
            next: 0,
            heap: self.heap,
        })
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(RuntimeMapAccess::from_fields(self.fields, self.heap))
    }
}

struct RuntimeTupleVariantSeqAccess<'de> {
    fields: &'de ScriptFields<Value>,
    next: usize,
    heap: &'de ScriptHeap,
}

impl<'de> SeqAccess<'de> for RuntimeTupleVariantSeqAccess<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        if self.next >= self.fields.len() {
            return Ok(None);
        }
        let field = self.next.to_string();
        self.next = self.next.saturating_add(1);
        let value = self
            .fields
            .get(&field)
            .ok_or_else(|| Error::custom("missing tuple variant field"))?;
        seed.deserialize(RuntimeValueDeserializer {
            value,
            heap: self.heap,
        })
        .map(Some)
    }
}
