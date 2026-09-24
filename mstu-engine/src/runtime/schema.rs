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

/// How deep a schema is described before it is taken on trust, matching the
/// depth the desktop is told about.
const MAX_DEPTH: usize = 8;

/// The shape one field must have, down to what a record or a list holds.
///
/// Owned, rather than read from the plugin's schema on the spot, because it
/// is kept beside a running node and used on every message.
#[derive(Clone)]
pub struct Shape {
    pub kind: ValueKind,

    /// A record's fields. None below `MAX_DEPTH`, where anything is accepted.
    pub fields: Option<Vec<Shape>>,

    /// What a list holds, None for the same reason.
    pub element: Option<Box<Shape>>,
}

/// The shape a message must have to satisfy `schema`. A null schema means
/// "no message", so nothing is expected.
pub fn shape_of(schema: *const Schema) -> Vec<Shape> {
    if schema.is_null() {
        return Vec::new();
    }

    fields_shape(unsafe { &*schema }, 1)
}

fn fields_shape(schema: &Schema, depth: usize) -> Vec<Shape> {
    unsafe { schema.fields.as_slice() }
        .iter()
        .map(|field| shape_of_type(field.ty, depth))
        .collect()
}

fn shape_of_type(ty: &Type, depth: usize) -> Shape {
    let mut shape = Shape {
        kind: kind_of(ty),
        fields: None,
        element: None,
    };

    if depth >= MAX_DEPTH {
        return shape;
    }

    match ty.kind {
        TypeKind::TypeRecord => {
            let record = unsafe { ty.schema.record_ };
            shape.fields = Some(fields_shape(record.schema, depth + 1));
        }

        TypeKind::TypeList => {
            let list = unsafe { ty.schema.list_ };
            shape.element = Some(Box::new(shape_of_type(list.element, depth + 1)));
        }

        _ => {}
    }

    shape
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
pub fn matches(message: &Message, expected: &[Shape]) -> bool {
    let values = message.values_slice();

    values.len() == expected.len() && values.iter().zip(expected).all(|(value, shape)| fits(value, shape))
}

/// A record is checked field by field, and a list element by element: a
/// wrongly shaped record is as fatal to a plugin as a wrongly typed field.
fn fits(value: &Value, shape: &Shape) -> bool {
    if value.kind != shape.kind {
        return false;
    }

    match value.kind {
        ValueKind::ValueRecord => match &shape.fields {
            Some(fields) => {
                let held = unsafe { value.data.record_.as_slice() };

                held.len() == fields.len()
                    && held.iter().zip(fields).all(|(value, shape)| fits(value, shape))
            }
            None => true,
        },

        ValueKind::ValueList => match &shape.element {
            Some(element) => unsafe { value.data.list_.as_slice() }
                .iter()
                .all(|value| fits(value, element)),
            None => true,
        },

        _ => true,
    }
}
