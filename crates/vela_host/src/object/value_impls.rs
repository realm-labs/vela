use vela_common::HostTypeId;

use crate::{
    call_value::HostCallValue, error::HostResult, target::HostTargetInstance, value::HostValue,
};

use super::errors::{invalid_arg, missing_target};
use super::target::target_is_leaf;
use super::{DetachedHostValue, HostValueFrom, HostValueInto, ScriptHostFieldAccess};

impl HostValueInto for bool {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(HostValue::Bool(self))
    }
}

impl HostValueFrom for bool {
    fn from_host_value(value: &HostValue) -> HostResult<Self> {
        match value {
            HostValue::Bool(value) => Ok(*value),
            _ => Err(invalid_arg("bool value")),
        }
    }
}

impl ScriptHostFieldAccess for bool {
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn script_host_type_shape() -> Option<String> {
        Some("bool".to_owned())
    }

    fn from_host_collection_value(value: HostValue) -> HostResult<Self> {
        bool::from_host_value(&value)
    }

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<HostValue> {
        if target_is_leaf(target, offset) {
            (*self).into_host_value()
        } else {
            Err(missing_target(target))
        }
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        if target_is_leaf(target, offset) {
            *self = bool::from_host_value(&value)?;
            Ok(())
        } else {
            Err(missing_target(target))
        }
    }
}

impl DetachedHostValue for bool {
    fn detached_host_type_shape() -> String {
        "bool".to_owned()
    }

    fn encode_detached_host_value(&self) -> HostResult<HostCallValue> {
        Ok(HostCallValue::Bool(*self))
    }

    fn decode_detached_host_value(value: &HostCallValue) -> HostResult<Self> {
        match value {
            HostCallValue::Bool(value) => Ok(*value),
            _ => Err(invalid_arg("bool")),
        }
    }
}

impl HostValueInto for String {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(HostValue::String(self))
    }
}

impl HostValueInto for &str {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(HostValue::String(self.to_owned()))
    }
}

impl HostValueFrom for String {
    fn from_host_value(value: &HostValue) -> HostResult<Self> {
        match value {
            HostValue::String(value) => Ok(value.clone()),
            _ => Err(invalid_arg("string value")),
        }
    }
}

impl HostValueInto for Vec<u8> {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(HostValue::Bytes(self))
    }
}

impl HostValueInto for &[u8] {
    fn into_host_value(self) -> HostResult<HostValue> {
        Ok(HostValue::Bytes(self.to_vec()))
    }
}

impl HostValueFrom for Vec<u8> {
    fn from_host_value(value: &HostValue) -> HostResult<Self> {
        match value {
            HostValue::Bytes(value) => Ok(value.clone()),
            _ => Err(invalid_arg("bytes")),
        }
    }
}

impl ScriptHostFieldAccess for String {
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn script_host_type_shape() -> Option<String> {
        Some("String".to_owned())
    }

    fn from_host_collection_value(value: HostValue) -> HostResult<Self> {
        String::from_host_value(&value)
    }

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<HostValue> {
        if target_is_leaf(target, offset) {
            self.as_str().into_host_value()
        } else {
            Err(missing_target(target))
        }
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        if target_is_leaf(target, offset) {
            *self = String::from_host_value(&value)?;
            Ok(())
        } else {
            Err(missing_target(target))
        }
    }
}

impl DetachedHostValue for String {
    fn detached_host_type_shape() -> String {
        "String".to_owned()
    }

    fn encode_detached_host_value(&self) -> HostResult<HostCallValue> {
        Ok(HostCallValue::String(self.clone()))
    }

    fn decode_detached_host_value(value: &HostCallValue) -> HostResult<Self> {
        match value {
            HostCallValue::String(value) => Ok(value.clone()),
            _ => Err(invalid_arg("String")),
        }
    }
}

impl<T> ScriptHostFieldAccess for Option<T>
where
    T: DetachedHostValue,
{
    fn script_host_type_id(&self) -> HostTypeId {
        HostTypeId::new(0)
    }

    fn script_host_type_shape() -> Option<String> {
        Some(format!("Option<{}>", T::detached_host_type_shape()))
    }

    fn from_host_collection_value(value: HostValue) -> HostResult<Self> {
        match value {
            HostValue::Detached(value) => Self::decode_detached_host_value(&value),
            value => Self::decode_detached_host_value(&HostCallValue::from_host_value(value)),
        }
    }

    fn read_host_target_from(
        &self,
        target: HostTargetInstance<'_>,
        offset: usize,
    ) -> HostResult<HostValue> {
        if target_is_leaf(target, offset) {
            Ok(HostValue::Detached(Box::new(
                self.encode_detached_host_value()?,
            )))
        } else {
            Err(missing_target(target))
        }
    }

    fn write_host_target_from(
        &mut self,
        target: HostTargetInstance<'_>,
        offset: usize,
        value: HostValue,
    ) -> HostResult<()> {
        if !target_is_leaf(target, offset) {
            return Err(missing_target(target));
        }
        *self = Self::from_host_collection_value(value)?;
        Ok(())
    }
}

impl<T> DetachedHostValue for Option<T>
where
    T: DetachedHostValue,
{
    fn detached_host_type_shape() -> String {
        format!("Option<{}>", T::detached_host_type_shape())
    }

    fn encode_detached_host_value(&self) -> HostResult<HostCallValue> {
        Ok(match self {
            Some(value) => HostCallValue::enum_variant(
                "Option",
                "Some",
                [("0", value.encode_detached_host_value()?)],
            ),
            None => HostCallValue::enum_variant(
                "Option",
                "None",
                std::iter::empty::<(&'static str, HostCallValue)>(),
            ),
        })
    }

    fn decode_detached_host_value(value: &HostCallValue) -> HostResult<Self> {
        match value {
            HostCallValue::Enum {
                enum_name,
                variant,
                fields,
            } if enum_name == "Option" && variant == "Some" => {
                let value = fields
                    .iter()
                    .find(|field| field.name == "0")
                    .ok_or_else(|| invalid_arg("Option::Some"))?;
                T::decode_detached_host_value(&value.value).map(Some)
            }
            HostCallValue::Enum {
                enum_name, variant, ..
            } if enum_name == "Option" && variant == "None" => Ok(None),
            _ => Err(invalid_arg("Option")),
        }
    }
}
