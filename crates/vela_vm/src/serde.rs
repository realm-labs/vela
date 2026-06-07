use std::collections::BTreeMap;
use std::fmt;

use ::serde::Serialize;
use ::serde::de::{
    self, DeserializeOwned, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use ::serde::ser::{
    self, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};

use crate::error::{VmError, VmErrorKind, VmResult};
use crate::owned_value::OwnedValue;
use crate::script_object::ScriptFields;

const OPERATION: &str = "serde owned value conversion";

#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    fn custom(message: impl fmt::Display) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Self::custom(msg)
    }
}

impl de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Self::custom(msg)
    }
}

impl From<Error> for VmError {
    fn from(_error: Error) -> Self {
        VmError::new(VmErrorKind::TypeMismatch {
            operation: OPERATION,
        })
    }
}

type Result<T> = std::result::Result<T, Error>;

pub fn to_owned_value<T>(value: &T) -> VmResult<OwnedValue>
where
    T: Serialize + ?Sized,
{
    value.serialize(OwnedValueSerializer).map_err(Into::into)
}

pub fn from_owned_value<T>(value: &OwnedValue) -> VmResult<T>
where
    T: DeserializeOwned,
{
    ::serde::Deserialize::deserialize(value).map_err(Into::into)
}

struct OwnedValueSerializer;

impl ser::Serializer for OwnedValueSerializer {
    type Ok = OwnedValue;
    type Error = Error;
    type SerializeSeq = SeqSerializer;
    type SerializeTuple = SeqSerializer;
    type SerializeTupleStruct = SeqSerializer;
    type SerializeTupleVariant = TupleVariantSerializer;
    type SerializeMap = MapSerializer;
    type SerializeStruct = StructValueSerializer;
    type SerializeStructVariant = StructVariantSerializer;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok> {
        Ok(OwnedValue::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok> {
        Ok(OwnedValue::Int(i64::from(v)))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok> {
        Ok(OwnedValue::Int(i64::from(v)))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok> {
        Ok(OwnedValue::Int(i64::from(v)))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok> {
        Ok(OwnedValue::Int(v))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok> {
        Ok(OwnedValue::Int(i64::from(v)))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok> {
        Ok(OwnedValue::Int(i64::from(v)))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok> {
        Ok(OwnedValue::Int(i64::from(v)))
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok> {
        let value = i64::try_from(v).map_err(|_| Error::custom("u64 does not fit in Vela Int"))?;
        Ok(OwnedValue::Int(value))
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok> {
        Ok(OwnedValue::Float(f64::from(v)))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok> {
        Ok(OwnedValue::Float(v))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok> {
        Ok(OwnedValue::String(v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok> {
        Ok(OwnedValue::String(v.to_owned()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok> {
        Ok(OwnedValue::Array(
            v.iter()
                .copied()
                .map(i64::from)
                .map(OwnedValue::Int)
                .collect(),
        ))
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        Ok(OwnedValue::Null)
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok> {
        Ok(OwnedValue::Null)
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok> {
        Ok(OwnedValue::record(name, Vec::<(String, OwnedValue)>::new()))
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok> {
        Ok(OwnedValue::enum_variant(
            name,
            variant,
            Vec::<(String, OwnedValue)>::new(),
        ))
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok>
    where
        T: Serialize + ?Sized,
    {
        Ok(OwnedValue::enum_variant(
            name,
            variant,
            [("0".to_owned(), value.serialize(OwnedValueSerializer)?)],
        ))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(SeqSerializer::new(len))
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        Ok(SeqSerializer::new(Some(len)))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Ok(SeqSerializer::new(Some(len)))
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Ok(TupleVariantSerializer {
            enum_name: name.to_owned(),
            variant: variant.to_owned(),
            values: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(MapSerializer {
            entries: BTreeMap::new(),
            next_key: None,
            len,
        })
    }

    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        Ok(StructValueSerializer {
            type_name: name.to_owned(),
            fields: Vec::with_capacity(len),
        })
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Ok(StructVariantSerializer {
            enum_name: name.to_owned(),
            variant: variant.to_owned(),
            fields: Vec::with_capacity(len),
        })
    }
}

struct SeqSerializer {
    values: Vec<OwnedValue>,
}

impl SeqSerializer {
    fn new(len: Option<usize>) -> Self {
        Self {
            values: Vec::with_capacity(len.unwrap_or(0)),
        }
    }
}

impl SerializeSeq for SeqSerializer {
    type Ok = OwnedValue;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        self.values.push(value.serialize(OwnedValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(OwnedValue::Array(self.values))
    }
}

impl SerializeTuple for SeqSerializer {
    type Ok = OwnedValue;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok> {
        SerializeSeq::end(self)
    }
}

impl SerializeTupleStruct for SeqSerializer {
    type Ok = OwnedValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok> {
        SerializeSeq::end(self)
    }
}

struct TupleVariantSerializer {
    enum_name: String,
    variant: String,
    values: Vec<OwnedValue>,
}

impl SerializeTupleVariant for TupleVariantSerializer {
    type Ok = OwnedValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        self.values.push(value.serialize(OwnedValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(OwnedValue::enum_variant(
            self.enum_name,
            self.variant,
            self.values
                .into_iter()
                .enumerate()
                .map(|(index, value)| (index.to_string(), value)),
        ))
    }
}

struct MapSerializer {
    entries: BTreeMap<String, OwnedValue>,
    next_key: Option<String>,
    len: Option<usize>,
}

impl SerializeMap for MapSerializer {
    type Ok = OwnedValue;
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        self.next_key = Some(key.serialize(MapKeySerializer)?);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        let key = self
            .next_key
            .take()
            .ok_or_else(|| Error::custom("map value serialized before key"))?;
        self.entries
            .insert(key, value.serialize(OwnedValueSerializer)?);
        Ok(())
    }

    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<()>
    where
        K: Serialize + ?Sized,
        V: Serialize + ?Sized,
    {
        let key = key.serialize(MapKeySerializer)?;
        self.entries
            .insert(key, value.serialize(OwnedValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        if self.len.is_some_and(|len| len != self.entries.len()) {
            return Err(Error::custom(
                "map serialized an unexpected number of entries",
            ));
        }
        Ok(OwnedValue::Map(self.entries))
    }
}

struct StructValueSerializer {
    type_name: String,
    fields: Vec<(String, OwnedValue)>,
}

impl SerializeStruct for StructValueSerializer {
    type Ok = OwnedValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        self.fields
            .push((key.to_owned(), value.serialize(OwnedValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(OwnedValue::Record {
            type_name: self.type_name.clone(),
            fields: ScriptFields::from_pairs(&self.type_name, self.fields),
        })
    }
}

struct StructVariantSerializer {
    enum_name: String,
    variant: String,
    fields: Vec<(String, OwnedValue)>,
}

impl SerializeStructVariant for StructVariantSerializer {
    type Ok = OwnedValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        self.fields
            .push((key.to_owned(), value.serialize(OwnedValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(OwnedValue::enum_variant(
            self.enum_name,
            self.variant,
            self.fields,
        ))
    }
}

struct MapKeySerializer;

impl ser::Serializer for MapKeySerializer {
    type Ok = String;
    type Error = Error;
    type SerializeSeq = ser::Impossible<String, Error>;
    type SerializeTuple = ser::Impossible<String, Error>;
    type SerializeTupleStruct = ser::Impossible<String, Error>;
    type SerializeTupleVariant = ser::Impossible<String, Error>;
    type SerializeMap = ser::Impossible<String, Error>;
    type SerializeStruct = ser::Impossible<String, Error>;
    type SerializeStructVariant = ser::Impossible<String, Error>;

    fn serialize_str(self, v: &str) -> Result<Self::Ok> {
        Ok(v.to_owned())
    }

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok> {
        Ok(v.to_string())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok>
    where
        T: Serialize + ?Sized,
    {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_unit(self) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok> {
        Ok(variant.to_owned())
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, _value: &T) -> Result<Self::Ok>
    where
        T: Serialize + ?Sized,
    {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok>
    where
        T: Serialize + ?Sized,
    {
        Ok(variant.to_owned())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(Error::custom("Vela maps require string serde keys"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::custom("Vela maps require string serde keys"))
    }
}

impl<'de> de::Deserializer<'de> for &'de OwnedValue {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self {
            OwnedValue::Missing | OwnedValue::Null => visitor.visit_unit(),
            OwnedValue::Bool(value) => visitor.visit_bool(*value),
            OwnedValue::Int(value) => visitor.visit_i64(*value),
            OwnedValue::Float(value) => visitor.visit_f64(*value),
            OwnedValue::String(value) => visitor.visit_str(value),
            OwnedValue::Array(values) | OwnedValue::Set(values) => {
                visitor.visit_seq(ValueSeqAccess {
                    iter: values.iter(),
                })
            }
            OwnedValue::Map(values) => visitor.visit_map(ValueMapAccess::from_map(values)),
            OwnedValue::Record { fields, .. } => {
                visitor.visit_map(ValueMapAccess::from_fields(fields))
            }
            OwnedValue::Enum { .. } => self.deserialize_enum("", &[], visitor),
            OwnedValue::Range(_)
            | OwnedValue::HostRef(_)
            | OwnedValue::PathProxy(_)
            | OwnedValue::Closure(_)
            | OwnedValue::Iterator(_) => Err(Error::custom("unsupported serde value")),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self {
            OwnedValue::Bool(value) => visitor.visit_bool(*value),
            _ => Err(Error::custom("expected bool")),
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(I64RangeVisitor::new(
            visitor,
            i64::from(i8::MIN),
            i64::from(i8::MAX),
        ))
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(I64RangeVisitor::new(
            visitor,
            i64::from(i16::MIN),
            i64::from(i16::MAX),
        ))
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(I64RangeVisitor::new(
            visitor,
            i64::from(i32::MIN),
            i64::from(i32::MAX),
        ))
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self {
            OwnedValue::Int(value) => visitor.visit_i64(*value),
            _ => Err(Error::custom("expected int")),
        }
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_u64(U64RangeVisitor::new(visitor, u64::from(u8::MAX)))
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_u64(U64RangeVisitor::new(visitor, u64::from(u16::MAX)))
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_u64(U64RangeVisitor::new(visitor, u64::from(u32::MAX)))
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self {
            OwnedValue::Int(value) if *value >= 0 => visitor.visit_u64(*value as u64),
            _ => Err(Error::custom("expected unsigned int")),
        }
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self {
            OwnedValue::Float(value) => visitor.visit_f32(*value as f32),
            OwnedValue::Int(value) => visitor.visit_f32(*value as f32),
            _ => Err(Error::custom("expected float")),
        }
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self {
            OwnedValue::Float(value) => visitor.visit_f64(*value),
            OwnedValue::Int(value) => visitor.visit_f64(*value as f64),
            _ => Err(Error::custom("expected float")),
        }
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self {
            OwnedValue::String(value) => {
                let mut chars = value.chars();
                match (chars.next(), chars.next()) {
                    (Some(value), None) => visitor.visit_char(value),
                    _ => Err(Error::custom("expected char")),
                }
            }
            _ => Err(Error::custom("expected char")),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self {
            OwnedValue::String(value) => visitor.visit_str(value),
            _ => Err(Error::custom("expected string")),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self {
            OwnedValue::Array(values) => {
                let bytes = values
                    .iter()
                    .map(|value| match value {
                        OwnedValue::Int(value) => {
                            u8::try_from(*value).map_err(|_| Error::custom("invalid byte"))
                        }
                        _ => Err(Error::custom("invalid byte")),
                    })
                    .collect::<Result<Vec<_>>>()?;
                visitor.visit_byte_buf(bytes)
            }
            _ => Err(Error::custom("expected bytes")),
        }
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self {
            OwnedValue::Missing | OwnedValue::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self {
            OwnedValue::Missing | OwnedValue::Null => visitor.visit_unit(),
            OwnedValue::Record { fields, .. } if fields.is_empty() => visitor.visit_unit(),
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
        match self {
            OwnedValue::Array(values) | OwnedValue::Set(values) => {
                visitor.visit_seq(ValueSeqAccess {
                    iter: values.iter(),
                })
            }
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
        match self {
            OwnedValue::Map(values) => visitor.visit_map(ValueMapAccess::from_map(values)),
            OwnedValue::Record { fields, .. } => {
                visitor.visit_map(ValueMapAccess::from_fields(fields))
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
        match self {
            OwnedValue::Record { fields, .. } => {
                visitor.visit_map(ValueMapAccess::from_fields(fields))
            }
            OwnedValue::Map(values) => visitor.visit_map(ValueMapAccess::from_map(values)),
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
        match self {
            OwnedValue::Enum {
                variant, fields, ..
            } => visitor.visit_enum(ValueEnumAccess { variant, fields }),
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
}

struct I64RangeVisitor<V> {
    inner: V,
    min: i64,
    max: i64,
}

impl<V> I64RangeVisitor<V> {
    fn new(inner: V, min: i64, max: i64) -> Self {
        Self { inner, min, max }
    }
}

impl<'de, V> Visitor<'de> for I64RangeVisitor<V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.expecting(formatter)
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        if (self.min..=self.max).contains(&value) {
            self.inner.visit_i64(value)
        } else {
            Err(E::custom("integer out of range"))
        }
    }
}

struct U64RangeVisitor<V> {
    inner: V,
    max: u64,
}

impl<V> U64RangeVisitor<V> {
    fn new(inner: V, max: u64) -> Self {
        Self { inner, max }
    }
}

impl<'de, V> Visitor<'de> for U64RangeVisitor<V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.expecting(formatter)
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value <= self.max {
            self.inner.visit_u64(value)
        } else {
            Err(E::custom("integer out of range"))
        }
    }
}

struct ValueSeqAccess<'de> {
    iter: std::slice::Iter<'de, OwnedValue>,
}

impl<'de> SeqAccess<'de> for ValueSeqAccess<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        self.iter
            .next()
            .map(|value| seed.deserialize(value))
            .transpose()
    }
}

struct ValueMapAccess<'de> {
    entries: Vec<(&'de str, &'de OwnedValue)>,
    next_value: Option<&'de OwnedValue>,
}

impl<'de> ValueMapAccess<'de> {
    fn from_map(values: &'de BTreeMap<String, OwnedValue>) -> Self {
        Self {
            entries: values
                .iter()
                .map(|(key, value)| (key.as_str(), value))
                .collect(),
            next_value: None,
        }
    }

    fn from_fields(fields: &'de ScriptFields<OwnedValue>) -> Self {
        Self {
            entries: fields.iter().collect(),
            next_value: None,
        }
    }
}

impl<'de> MapAccess<'de> for ValueMapAccess<'de> {
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
        seed.deserialize(value)
    }
}

struct ValueEnumAccess<'de> {
    variant: &'de str,
    fields: &'de ScriptFields<OwnedValue>,
}

impl<'de> EnumAccess<'de> for ValueEnumAccess<'de> {
    type Error = Error;
    type Variant = ValueVariantAccess<'de>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: de::DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(self.variant.into_deserializer())?;
        Ok((
            variant,
            ValueVariantAccess {
                fields: self.fields,
            },
        ))
    }
}

struct ValueVariantAccess<'de> {
    fields: &'de ScriptFields<OwnedValue>,
}

impl<'de> VariantAccess<'de> for ValueVariantAccess<'de> {
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
        seed.deserialize(value)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(TupleVariantSeqAccess {
            fields: self.fields,
            next: 0,
        })
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(ValueMapAccess::from_fields(self.fields))
    }
}

struct TupleVariantSeqAccess<'de> {
    fields: &'de ScriptFields<OwnedValue>,
    next: usize,
}

impl<'de> SeqAccess<'de> for TupleVariantSeqAccess<'de> {
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
        seed.deserialize(value).map(Some)
    }
}
