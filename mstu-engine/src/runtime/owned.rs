use mstu_sdk::{Message, Value, ValueKind};

/// A message value copied out of the ABI so it can outlive the plugin call.
#[derive(Debug, Clone, PartialEq)]
pub enum Owned {
    None,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Str(String),
    Bytes(usize),
    List(Vec<Owned>),
}

pub fn own(value: &Value) -> Owned {
    unsafe {
        match value.kind {
            ValueKind::ValueNone => Owned::None,
            ValueKind::ValueBool => Owned::Bool(value.data.bool_),
            ValueKind::ValueInt => Owned::Int(value.data.int_),
            ValueKind::ValueUint => Owned::Uint(value.data.uint_),
            ValueKind::ValueFloat => Owned::Float(value.data.float_),
            ValueKind::ValueString => Owned::Str(value.data.string_.as_str().to_string()),

            // Payloads are for display: keep the size, not the bytes.
            ValueKind::ValueBytes => Owned::Bytes(value.data.bytes_.len),

            ValueKind::ValueList | ValueKind::ValueRecord => {
                Owned::List(value.data.list_.as_slice().iter().map(own).collect())
            }
        }
    }
}

pub fn own_message(message: &Message) -> Vec<Owned> {
    message.values_slice().iter().map(own).collect()
}
