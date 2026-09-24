use std::sync::Arc;

use mstu_sdk::{Message, Slice, Value, ValueKind, message};

use crate::runtime::{
    alloc::{self, Arena},
    schema::Shape,
    value::take_value,
};

pub struct ProcessData {
    pub output: Message,
    pub arena: Arena,

    /// Messages this one points into. A mapped message carries the values of
    /// the message it was mapped from, so that one has to outlive it.
    keep: Vec<Arc<ProcessData>>,
}

unsafe impl Send for ProcessData {}

/// Shared only after it is finished, and read-only from then on: the router
/// hands the same message to every target, and a target only reads its input.
unsafe impl Sync for ProcessData {}

impl ProcessData {
    /// Creates a message of `field_count` `none` fields.
    ///
    /// The values array itself is allocated from the arena, so the whole
    /// message (values + every buffer a plugin fills in) lives in one
    /// allocation owned by this `ProcessData`.
    pub fn new(field_count: usize) -> Self {
        let mut arena = alloc::acquire();
        let values = arena.alloc::<Value>(field_count);

        for index in 0..field_count {
            unsafe { values.add(index).write(Value::none()) };
        }

        Self {
            output: Message {
                values: Slice::from_raw_parts(values, field_count),
            },
            arena,
            keep: Vec::new(),
        }
    }

    /// Creates the message `shape` describes, with every field none.
    ///
    /// Records are built out to their own fields, so a mapping can reach a
    /// field inside one: nothing else allocates the record it lands in.
    pub fn from_shape(shape: &[Shape]) -> Self {
        let mut arena = alloc::acquire();
        let values = build(&mut arena, shape);

        Self {
            output: Message { values },
            arena,
            keep: Vec::new(),
        }
    }

    /// Keeps `source` alive for as long as this message lives, because its
    /// values point into it.
    pub fn borrows_from(&mut self, source: Arc<ProcessData>) {
        self.keep.push(source);
    }

    /// Whether nothing was written: a record counts as written only if one of
    /// its own fields is.
    pub fn is_empty(&self) -> bool {
        self.output.values_slice().iter().all(untouched)
    }
}

impl Drop for ProcessData {
    fn drop(&mut self) {
        // The message lives in the arena, so both die here.
        alloc::release(std::mem::replace(&mut self.arena, Arena::new(0)));
    }
}

/// Lays out one level of a message in the arena, records and all.
fn build(arena: &mut Arena, shape: &[Shape]) -> Slice<Value> {
    if shape.is_empty() {
        return Slice::empty();
    }

    let values = arena.alloc::<Value>(shape.len());

    for (index, field) in shape.iter().enumerate() {
        let value = match (field.kind, &field.fields) {
            (ValueKind::ValueRecord, Some(fields)) => Value::record(build(arena, fields)),
            _ => Value::none(),
        };

        unsafe { values.add(index).write(value) };
    }

    Slice::from_raw_parts(values, shape.len())
}

fn untouched(value: &Value) -> bool {
    match value.kind {
        ValueKind::ValueNone => true,
        ValueKind::ValueRecord => unsafe { value.data.record_.as_slice() }
            .iter()
            .all(untouched),
        _ => false,
    }
}

/// Creates a writer over a message.
///
/// `_engine_data` must be set to the `ProcessData` the writer fills in
/// before it is handed to a plugin.
pub fn new_writer() -> message::Writer {
    message::Writer {
        _engine_data: std::ptr::null_mut(),

        fill,
        reserve,
    }
}

unsafe fn process_data(writer: *mut message::Writer) -> &'static mut ProcessData {
    unsafe { &mut *((*writer)._engine_data as *mut ProcessData) }
}

unsafe fn values_mut(writer: *mut message::Writer) -> &'static mut [Value] {
    let process_data = unsafe { process_data(writer) };

    unsafe {
        std::slice::from_raw_parts_mut(
            process_data.output.values.ptr as *mut Value,
            process_data.output.values.len,
        )
    }
}
/// Writes the whole message the plugin built.
pub extern "C" fn fill(writer: *mut message::Writer, message: Message) -> bool {
    let process_data = unsafe { process_data(writer) };
    let values = unsafe { values_mut(writer) };
    let given = message.values_slice();

    // One value per field: a short message would leave fields unwritten.
    if given.len() != values.len() {
        return false;
    }

    for (slot, value) in values.iter_mut().zip(given) {
        *slot = unsafe { take_value(&mut process_data.arena, value) };
    }

    true
}

/// Engine memory for a payload, so the plugin writes it once and in place.
pub extern "C" fn reserve(writer: *mut message::Writer, len: usize) -> *mut u8 {
    let process_data = unsafe { process_data(writer) };

    process_data.arena.alloc::<u8>(len)
}

#[cfg(test)]
mod tests {
    use mstu_sdk::{Field, Schema, Str, TYPE_BYTES, TYPE_U64, slice};

    use super::*;
    use crate::runtime::schema;

    static FIELDS: [Field; 2] = [
        Field {
            name: Str::from_static("id"),
            ty: &TYPE_U64,
            description: Str::empty(),
        },
        Field {
            name: Str::from_static("data"),
            ty: &TYPE_BYTES,
            description: Str::empty(),
        },
    ];

    static SCHEMA: Schema = Schema {
        fields: slice!(FIELDS),
    };

    fn writer_for(data: &mut ProcessData) -> message::Writer {
        let mut writer = new_writer();
        writer._engine_data = (data as *mut ProcessData).cast();
        writer
    }

    /// A payload written into engine memory travels as it lies.
    #[test]
    fn keeps_what_was_written_into_reserved_memory() {
        let mut data = ProcessData::from_shape(&schema::shape_of(&SCHEMA));
        let mut writer = writer_for(&mut data);

        let buffer = writer.reserve(4).expect("reserved");
        buffer.copy_from_slice(&[1, 2, 3, 4]);

        let payload = Slice::from_raw_parts(buffer.as_ptr(), buffer.len());
        assert!(writer.fill(&[7u64.into(), payload.into()]));

        let value = data.output.values_slice()[1];

        assert!(value.kind == ValueKind::ValueBytes);
        assert_eq!(unsafe { value.data.bytes_.as_slice() }, &[1, 2, 3, 4]);

        // Taken as it lies, not copied a second time.
        assert_eq!(unsafe { value.data.bytes_.ptr }, payload.ptr);
    }

    /// A buffer of the plugin's own is copied, so the message owns it.
    #[test]
    fn copies_what_the_plugin_still_owns() {
        let mut data = ProcessData::from_shape(&schema::shape_of(&SCHEMA));
        let mut writer = writer_for(&mut data);

        let held = vec![9u8; 32];
        let payload = Slice::from_raw_parts(held.as_ptr(), held.len());

        assert!(writer.fill(&[1u64.into(), payload.into()]));

        let value = data.output.values_slice()[1];

        assert_ne!(unsafe { value.data.bytes_.ptr }, payload.ptr);
        assert_eq!(unsafe { value.data.bytes_.as_slice() }, held.as_slice());
    }

    /// One value per field, or nothing is written.
    #[test]
    fn refuses_the_wrong_number_of_fields() {
        let mut data = ProcessData::from_shape(&schema::shape_of(&SCHEMA));
        let mut writer = writer_for(&mut data);

        assert!(!writer.fill(&[1u64.into()]));
        assert!(data.is_empty());
    }

    /// A borrowed buffer outlives the message that borrowed it, not the one
    /// that owns it.
    #[test]
    fn a_borrowed_message_holds_its_source_up() {
        let source = Arc::new(ProcessData::from_shape(&schema::shape_of(&SCHEMA)));
        let mut target = ProcessData::from_shape(&schema::shape_of(&SCHEMA));

        target.borrows_from(source.clone());
        assert_eq!(Arc::strong_count(&source), 2);

        drop(target);
        assert_eq!(Arc::strong_count(&source), 1);
    }
}
