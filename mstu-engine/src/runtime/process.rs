use mstu_sdk::{Message, Slice, Str, Value, ValueKind, message};

use crate::runtime::alloc::{self, Arena};

pub struct ProcessData {
    pub output: Message,
    pub arena: Arena,
}

unsafe impl Send for ProcessData {}

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
        }
    }

    pub fn is_empty(&self) -> bool {
        self.output
            .values_slice()
            .iter()
            .all(|value| value.kind == ValueKind::ValueNone)
    }
}

impl Drop for ProcessData {
    fn drop(&mut self) {
        // The message lives in the arena, so both die here.
        alloc::release(std::mem::replace(&mut self.arena, Arena::new(0)));
    }
}

/// Creates a writer backed by the engine setters.
///
/// `_engine_data` must be set to the `ProcessData` the writer fills in
/// before it is handed to a plugin.
pub fn new_writer() -> message::Writer {
    message::Writer {
        _engine_data: std::ptr::null_mut(),

        set_none,
        set_bool,
        set_int,
        set_uint,
        set_float,
        set_str,
        set_bytes,
        set_list,
        set_record,
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
unsafe fn clone_slice<T: Copy>(arena: &mut alloc::Arena, slice: Slice<T>) -> Slice<T> {
    if slice.ptr.is_null() || slice.len == 0 {
        return Slice::empty();
    }

    let dst = arena.alloc::<T>(slice.len);
    unsafe {
        std::ptr::copy_nonoverlapping(slice.ptr, dst, slice.len);
    }

    Slice::from_raw_parts(dst, slice.len)
}

unsafe fn clone_value(arena: &mut alloc::Arena, value: &Value) -> Value {
    match value.kind {
        ValueKind::ValueNone => Value::none(),

        ValueKind::ValueBool
        | ValueKind::ValueInt
        | ValueKind::ValueUint
        | ValueKind::ValueFloat
        | ValueKind::ValueString => *value,

        ValueKind::ValueBytes => unsafe { clone_slice(arena, value.data.bytes_).into() },

        ValueKind::ValueList => unsafe {
            let src = value.data.list_.as_slice();
            let dst = arena.alloc::<Value>(src.len());

            for (i, v) in src.iter().enumerate() {
                dst.add(i).write(clone_value(arena, v));
            }

            Slice::from_raw_parts(dst, src.len()).into()
        },

        ValueKind::ValueRecord => unsafe {
            let src = value.data.record_.as_slice();
            let dst = arena.alloc::<Value>(src.len());

            for (i, v) in src.iter().enumerate() {
                dst.add(i).write(clone_value(arena, v));
            }

            Slice::from_raw_parts(dst, src.len()).into()
        },
    }
}

macro_rules! impl_primitive_setter {
    ($name:ident, $ty:ty) => {
        pub extern "C" fn $name(writer: *mut message::Writer, field: usize, value: $ty) -> bool {
            let values = unsafe { values_mut(writer) };

            if field >= values.len() {
                return false;
            }

            values[field] = value.into();
            true
        }
    };
}

impl_primitive_setter!(set_bool, bool);
impl_primitive_setter!(set_int, i64);
impl_primitive_setter!(set_uint, u64);
impl_primitive_setter!(set_float, f64);
impl_primitive_setter!(set_str, Str);

pub extern "C" fn set_bytes(writer: *mut message::Writer, field: usize, value: Slice<u8>) -> bool {
    let process_data = unsafe { process_data(writer) };
    let values = unsafe { values_mut(writer) };

    if field >= values.len() {
        return false;
    }

    unsafe {
        values[field] = clone_slice(&mut process_data.arena, value).into();
    }

    true
}

pub extern "C" fn set_list(
    writer: *mut message::Writer,
    field: usize,
    value: Slice<Value>,
) -> bool {
    let process_data = unsafe { process_data(writer) };
    let values = unsafe { values_mut(writer) };

    if field >= values.len() {
        return false;
    }

    unsafe {
        let src = value.as_slice();
        let dst = process_data.arena.alloc::<Value>(src.len());

        for (i, v) in src.iter().enumerate() {
            dst.add(i).write(clone_value(&mut process_data.arena, v));
        }

        values[field] = Slice::from_raw_parts(dst, src.len()).into();
    }

    true
}

pub extern "C" fn set_record(
    writer: *mut message::Writer,
    field: usize,
    value: Slice<Value>,
) -> bool {
    set_list(writer, field, value)
}

pub extern "C" fn set_none(writer: *mut message::Writer, field: usize) -> bool {
    let values = unsafe { values_mut(writer) };

    if field >= values.len() {
        return false;
    }

    values[field] = Value::none();
    true
}
