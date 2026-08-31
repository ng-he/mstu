use mstu_sdk::{Message, Schema, Type, TypeKind, ValueKind};

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
