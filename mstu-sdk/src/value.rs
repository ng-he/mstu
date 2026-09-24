use std::{fmt, write};

use crate::{Slice, Str};

#[repr(C)]
#[derive(Copy, Clone, PartialEq)]
pub enum ValueKind {
    ValueNone,
    ValueBool,
    ValueInt,
    ValueUint,
    ValueFloat,
    ValueString,
    ValueList,
    ValueRecord,
    ValueBytes,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union Data {
    pub raw_: u64,
    pub bool_: bool,
    pub int_: i64,
    pub uint_: u64,
    pub float_: f64,
    pub string_: Str,
    pub list_: Slice<Value>,
    pub record_: Slice<Value>,
    pub bytes_: Slice<u8>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Value {
    pub kind: ValueKind,
    pub data: Data,
}

impl Default for Value {
    fn default() -> Self {
        Self {
            kind: ValueKind::ValueNone,
            data: Data { raw_: 0 },
        }
    }
}

unsafe impl Sync for Slice<u8> {}
unsafe impl Send for Slice<u8> {}

unsafe impl Sync for Slice<Value> {}
unsafe impl Send for Slice<Value> {}

pub trait FromValue: Sized {
    fn from_value(value: &Value) -> Option<Self>;
}

impl Value {
    #[inline]
    pub fn get<T: FromValue>(&self) -> Option<T> {
        T::from_value(self)
    }
}

impl Value {
    #[inline]
    pub const fn none() -> Self {
        Self {
            kind: ValueKind::ValueNone,
            data: Data { uint_: 0 },
        }
    }

    #[inline]
    pub const fn list(values: Slice<Value>) -> Self {
        Self {
            kind: ValueKind::ValueList,
            data: Data { list_: values },
        }
    }

    /// A record and a list carry the same slice, so `From` cannot tell them
    /// apart: a record has to be built by name.
    #[inline]
    pub const fn record(fields: Slice<Value>) -> Self {
        Self {
            kind: ValueKind::ValueRecord,
            data: Data { record_: fields },
        }
    }
}

impl FromValue for bool {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueBool {
            return None;
        }

        Some(unsafe { value.data.bool_ })
    }
}

impl FromValue for i64 {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueInt {
            return None;
        }

        Some(unsafe { value.data.int_ })
    }
}

impl FromValue for u64 {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueUint {
            return None;
        }

        Some(unsafe { value.data.uint_ })
    }
}

impl FromValue for f64 {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueFloat {
            return None;
        }

        Some(unsafe { value.data.float_ })
    }
}

impl FromValue for Str {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueString {
            return None;
        }

        Some(unsafe { value.data.string_ })
    }
}

impl FromValue for Slice<Value> {
    fn from_value(value: &Value) -> Option<Self> {
        match value.kind {
            ValueKind::ValueList => Some(unsafe { value.data.list_ }),
            ValueKind::ValueRecord => Some(unsafe { value.data.record_ }),
            _ => None,
        }
    }
}

impl FromValue for Slice<u8> {
    fn from_value(value: &Value) -> Option<Self> {
        if value.kind != ValueKind::ValueBytes {
            return None;
        }

        Some(unsafe { value.data.bytes_ })
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self {
            kind: ValueKind::ValueBool,
            data: Data { bool_: value },
        }
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self {
            kind: ValueKind::ValueInt,
            data: Data { int_: value },
        }
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Self {
            kind: ValueKind::ValueUint,
            data: Data { uint_: value },
        }
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self {
            kind: ValueKind::ValueFloat,
            data: Data { float_: value },
        }
    }
}

impl From<Str> for Value {
    fn from(value: Str) -> Self {
        Self {
            kind: ValueKind::ValueString,
            data: Data { string_: value },
        }
    }
}

impl From<Slice<Value>> for Value {
    fn from(value: Slice<Value>) -> Self {
        Self {
            kind: ValueKind::ValueList,
            data: Data { list_: value },
        }
    }
}

impl From<Slice<u8>> for Value {
    fn from(value: Slice<u8>) -> Self {
        Self {
            kind: ValueKind::ValueBytes,
            data: Data { bytes_: value },
        }
    }
}

const MAX_BYTES_PREVIEW: usize = 16;

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            match self.kind {
                ValueKind::ValueNone => {
                    write!(f, "none")
                }
                ValueKind::ValueBool => {
                    write!(f, "{}", self.data.bool_)
                }

                ValueKind::ValueInt => {
                    write!(f, "{}", self.data.int_)
                }

                ValueKind::ValueUint => {
                    write!(f, "{}", self.data.uint_)
                }

                ValueKind::ValueFloat => {
                    write!(f, "{}", self.data.float_)
                }

                ValueKind::ValueString => {
                    write!(f, "\"{}\"", self.data.string_.as_str())
                }

                ValueKind::ValueBytes => {
                    let bytes = self.data.bytes_.as_slice();

                    write!(f, "[len={}, data=[", bytes.len())?;
                    for (i, byte) in bytes.iter().take(MAX_BYTES_PREVIEW).enumerate() {
                        if i != 0 {
                            write!(f, " ")?;
                        }

                        write!(f, "{:02X}", byte)?;
                    }

                    if bytes.len() > MAX_BYTES_PREVIEW {
                        write!(f, " ...")?;
                    }

                    write!(f, "]]")
                }

                ValueKind::ValueList => {
                    let list = self.data.list_.as_slice();

                    write!(f, "[")?;

                    for (i, value) in list.iter().enumerate() {
                        if i != 0 {
                            write!(f, ", ")?;
                        }

                        write!(f, "{value}")?;
                    }

                    write!(f, "]")
                }

                ValueKind::ValueRecord => {
                    let record = self.data.record_.as_slice();

                    write!(f, "{{")?;

                    for (i, value) in record.iter().enumerate() {
                        if i != 0 {
                            write!(f, ", ")?;
                        }

                        write!(f, "{value}")?;
                    }

                    write!(f, "}}")
                }
            }
        }
    }
}
