use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PrimitiveTag {
    Null,
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    String,
    Bytes,
}

impl PrimitiveTag {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            PrimitiveTag::Null => "null",
            PrimitiveTag::Bool => "bool",
            PrimitiveTag::I8 => "i8",
            PrimitiveTag::I16 => "i16",
            PrimitiveTag::I32 => "i32",
            PrimitiveTag::I64 => "i64",
            PrimitiveTag::U8 => "u8",
            PrimitiveTag::U16 => "u16",
            PrimitiveTag::U32 => "u32",
            PrimitiveTag::U64 => "u64",
            PrimitiveTag::F32 => "f32",
            PrimitiveTag::F64 => "f64",
            PrimitiveTag::String => "string",
            PrimitiveTag::Bytes => "bytes",
        }
    }

    #[must_use]
    pub const fn numeric_tag(self) -> Option<NumericTag> {
        match self {
            PrimitiveTag::I8 => Some(NumericTag::I8),
            PrimitiveTag::I16 => Some(NumericTag::I16),
            PrimitiveTag::I32 => Some(NumericTag::I32),
            PrimitiveTag::I64 => Some(NumericTag::I64),
            PrimitiveTag::U8 => Some(NumericTag::U8),
            PrimitiveTag::U16 => Some(NumericTag::U16),
            PrimitiveTag::U32 => Some(NumericTag::U32),
            PrimitiveTag::U64 => Some(NumericTag::U64),
            PrimitiveTag::F32 => Some(NumericTag::F32),
            PrimitiveTag::F64 => Some(NumericTag::F64),
            PrimitiveTag::Null
            | PrimitiveTag::Bool
            | PrimitiveTag::String
            | PrimitiveTag::Bytes => None,
        }
    }
}

impl fmt::Display for PrimitiveTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NumericTag {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}

impl NumericTag {
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.primitive_tag().name()
    }

    #[must_use]
    pub const fn primitive_tag(self) -> PrimitiveTag {
        match self {
            NumericTag::I8 => PrimitiveTag::I8,
            NumericTag::I16 => PrimitiveTag::I16,
            NumericTag::I32 => PrimitiveTag::I32,
            NumericTag::I64 => PrimitiveTag::I64,
            NumericTag::U8 => PrimitiveTag::U8,
            NumericTag::U16 => PrimitiveTag::U16,
            NumericTag::U32 => PrimitiveTag::U32,
            NumericTag::U64 => PrimitiveTag::U64,
            NumericTag::F32 => PrimitiveTag::F32,
            NumericTag::F64 => PrimitiveTag::F64,
        }
    }

    #[must_use]
    pub const fn is_signed_integer(self) -> bool {
        matches!(
            self,
            NumericTag::I8 | NumericTag::I16 | NumericTag::I32 | NumericTag::I64
        )
    }

    #[must_use]
    pub const fn is_unsigned_integer(self) -> bool {
        matches!(
            self,
            NumericTag::U8 | NumericTag::U16 | NumericTag::U32 | NumericTag::U64
        )
    }

    #[must_use]
    pub const fn is_integer(self) -> bool {
        self.is_signed_integer() || self.is_unsigned_integer()
    }

    #[must_use]
    pub const fn is_float(self) -> bool {
        matches!(self, NumericTag::F32 | NumericTag::F64)
    }
}

impl fmt::Display for NumericTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScalarValue {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
}

impl ScalarValue {
    #[must_use]
    pub const fn numeric_tag(self) -> NumericTag {
        match self {
            ScalarValue::I8(_) => NumericTag::I8,
            ScalarValue::I16(_) => NumericTag::I16,
            ScalarValue::I32(_) => NumericTag::I32,
            ScalarValue::I64(_) => NumericTag::I64,
            ScalarValue::U8(_) => NumericTag::U8,
            ScalarValue::U16(_) => NumericTag::U16,
            ScalarValue::U32(_) => NumericTag::U32,
            ScalarValue::U64(_) => NumericTag::U64,
            ScalarValue::F32(_) => NumericTag::F32,
            ScalarValue::F64(_) => NumericTag::F64,
        }
    }

    #[must_use]
    pub const fn primitive_tag(self) -> PrimitiveTag {
        self.numeric_tag().primitive_tag()
    }

    #[must_use]
    pub const fn type_name(self) -> &'static str {
        self.primitive_tag().name()
    }
}

impl fmt::Display for ScalarValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScalarValue::I8(value) => write!(formatter, "{value}i8"),
            ScalarValue::I16(value) => write!(formatter, "{value}i16"),
            ScalarValue::I32(value) => write!(formatter, "{value}i32"),
            ScalarValue::I64(value) => write!(formatter, "{value}i64"),
            ScalarValue::U8(value) => write!(formatter, "{value}u8"),
            ScalarValue::U16(value) => write!(formatter, "{value}u16"),
            ScalarValue::U32(value) => write!(formatter, "{value}u32"),
            ScalarValue::U64(value) => write!(formatter, "{value}u64"),
            ScalarValue::F32(value) => write!(formatter, "{value}f32"),
            ScalarValue::F64(value) => write!(formatter, "{value}f64"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMITIVE_NAMES: &[(PrimitiveTag, &str)] = &[
        (PrimitiveTag::Null, "null"),
        (PrimitiveTag::Bool, "bool"),
        (PrimitiveTag::I8, "i8"),
        (PrimitiveTag::I16, "i16"),
        (PrimitiveTag::I32, "i32"),
        (PrimitiveTag::I64, "i64"),
        (PrimitiveTag::U8, "u8"),
        (PrimitiveTag::U16, "u16"),
        (PrimitiveTag::U32, "u32"),
        (PrimitiveTag::U64, "u64"),
        (PrimitiveTag::F32, "f32"),
        (PrimitiveTag::F64, "f64"),
        (PrimitiveTag::String, "string"),
        (PrimitiveTag::Bytes, "bytes"),
    ];

    const NUMERIC_NAMES: &[(NumericTag, &str)] = &[
        (NumericTag::I8, "i8"),
        (NumericTag::I16, "i16"),
        (NumericTag::I32, "i32"),
        (NumericTag::I64, "i64"),
        (NumericTag::U8, "u8"),
        (NumericTag::U16, "u16"),
        (NumericTag::U32, "u32"),
        (NumericTag::U64, "u64"),
        (NumericTag::F32, "f32"),
        (NumericTag::F64, "f64"),
    ];

    #[test]
    fn primitive_tags_have_canonical_names() {
        for (tag, name) in PRIMITIVE_NAMES {
            assert_eq!(tag.name(), *name);
            assert_eq!(tag.to_string(), *name);
        }
    }

    #[test]
    fn numeric_tags_map_to_primitive_tags() {
        for (tag, name) in NUMERIC_NAMES {
            assert_eq!(tag.name(), *name);
            assert_eq!(tag.to_string(), *name);
            assert_eq!(tag.primitive_tag().name(), *name);
            assert_eq!(tag.primitive_tag().numeric_tag(), Some(*tag));
        }

        assert_eq!(PrimitiveTag::Null.numeric_tag(), None);
        assert_eq!(PrimitiveTag::Bool.numeric_tag(), None);
        assert_eq!(PrimitiveTag::String.numeric_tag(), None);
        assert_eq!(PrimitiveTag::Bytes.numeric_tag(), None);
    }

    #[test]
    fn numeric_tags_classify_domains() {
        assert!(NumericTag::I8.is_signed_integer());
        assert!(NumericTag::I64.is_integer());
        assert!(NumericTag::U32.is_unsigned_integer());
        assert!(NumericTag::U64.is_integer());
        assert!(NumericTag::F32.is_float());
        assert!(NumericTag::F64.is_float());

        assert!(!NumericTag::F64.is_integer());
        assert!(!NumericTag::I16.is_float());
        assert!(!NumericTag::U8.is_signed_integer());
    }

    #[test]
    fn scalar_values_report_tags_and_type_names() {
        let values = [
            (ScalarValue::I8(-1), NumericTag::I8, "i8", "-1i8"),
            (ScalarValue::I16(-2), NumericTag::I16, "i16", "-2i16"),
            (ScalarValue::I32(-3), NumericTag::I32, "i32", "-3i32"),
            (ScalarValue::I64(-4), NumericTag::I64, "i64", "-4i64"),
            (ScalarValue::U8(1), NumericTag::U8, "u8", "1u8"),
            (ScalarValue::U16(2), NumericTag::U16, "u16", "2u16"),
            (ScalarValue::U32(3), NumericTag::U32, "u32", "3u32"),
            (ScalarValue::U64(4), NumericTag::U64, "u64", "4u64"),
            (ScalarValue::F32(1.5), NumericTag::F32, "f32", "1.5f32"),
            (ScalarValue::F64(2.5), NumericTag::F64, "f64", "2.5f64"),
        ];

        for (value, numeric_tag, type_name, display) in values {
            assert_eq!(value.numeric_tag(), numeric_tag);
            assert_eq!(value.primitive_tag(), numeric_tag.primitive_tag());
            assert_eq!(value.type_name(), type_name);
            assert_eq!(value.to_string(), display);
        }
    }
}
