use mstu_sdk::{Message, Schema, Type, TypeKind, Value, ValueKind};

/// The value kind a field of this type must carry.
fn kind_of(ty: &Type) -> ValueKind {
    match ty.kind {
        TypeKind::TypeBool => ValueKind::ValueBool,
        TypeKind::TypeInt => {
            if unsafe { ty.schema.int_ }.signed {
                ValueKind::ValueInt
            } else {
                ValueKind::ValueUint
            }
        }
        TypeKind::TypeFloat => ValueKind::ValueFloat,
        // Enum variants travel as their string value.
        TypeKind::TypeString | TypeKind::TypeEnum => ValueKind::ValueString,
        TypeKind::TypeBytes => ValueKind::ValueBytes,
        TypeKind::TypeList => ValueKind::ValueList,
        TypeKind::TypeRecord => ValueKind::ValueRecord,
    }
}

/// The kinds a message must carry to satisfy `schema`. A null schema means
/// "no message", so nothing is expected.
pub fn expected_kinds(schema: *const Schema) -> Vec<ValueKind> {
    if schema.is_null() {
        return Vec::new();
    }

    unsafe { (*schema).fields.as_slice() }
        .iter()
        .map(|field| kind_of(field.ty))
        .collect()
}

/// Retypes a number to the kind its field declares.
///
/// JSON has one number type, so `0` arrives as a uint even where the schema
/// asks for a float, and a plugin reading it by the declared type sees nothing.
pub fn coerce(value: Value, expected: ValueKind) -> Value {
    if value.kind == expected {
        return value;
    }

    unsafe {
        match (expected, value.kind) {
            (ValueKind::ValueFloat, ValueKind::ValueUint) => (value.data.uint_ as f64).into(),
            (ValueKind::ValueFloat, ValueKind::ValueInt) => (value.data.int_ as f64).into(),
            (ValueKind::ValueUint, ValueKind::ValueInt) if value.data.int_ >= 0 => {
                (value.data.int_ as u64).into()
            }
            (ValueKind::ValueInt, ValueKind::ValueUint) => (value.data.uint_ as i64).into(),
            _ => value,
        }
    }
}

/// The kind a single field of `schema` demands.
pub fn field_kind(schema: *const Schema, field: usize) -> Option<ValueKind> {
    if schema.is_null() {
        return None;
    }

    unsafe { (*schema).fields.as_slice() }
        .get(field)
        .map(|field| kind_of(field.ty))
}

/// Plugins read their input by schema and unwrap, so a message that does not
/// match is a crash waiting to happen. The engine never delivers one.
pub fn matches(message: &Message, expected: &[ValueKind]) -> bool {
    let values = message.values_slice();

    values.len() == expected.len()
        && values
            .iter()
            .zip(expected)
            .all(|(value, kind)| value.kind == *kind)
}
