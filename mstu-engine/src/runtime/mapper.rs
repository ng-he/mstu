use mstu_sdk as sdk;
use sdk::{Message, Value, ValueKind};

use crate::runtime::alloc::Arena;

#[derive(Debug, Clone)]
pub struct Mapping {
    /// Path to the source value in the input message.
    ///
    /// Example: [2, 4, 1] means:
    /// input[2].record[4].record[1]
    pub input: Vec<usize>,

    /// Path to the destination value in the output message.
    ///
    /// Example: [0, 3, 2] means:
    /// output[0].record[3].record[2]
    pub output: Vec<usize>,
}

#[derive(Debug, Default)]
pub struct Mapper {
    mappings: Vec<Mapping>,
}

impl Mapper {
    pub fn new() -> Self {
        Self {
            mappings: Vec::new(),
        }
    }

    pub fn add(&mut self, mapping: Mapping) {
        self.mappings.push(mapping);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Mapping> {
        self.mappings.iter()
    }

    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    pub fn map(&self, input: &Message, output: &mut Message, arena: &mut Arena) -> bool {
        for mapping in &self.mappings {
            let Some(src) = get_message_value(input, &mapping.input) else {
                return false;
            };

            let Some(dst) = get_message_value_mut(output, &mapping.output) else {
                return false;
            };

            let cloned = unsafe { clone_value(arena, src) };

            *dst = cloned;
        }

        true
    }
}

/// Gets a value from a message using a path.
///
/// An empty path means the root message is not a valid Value,
/// so this function returns None.
///
/// Example:
///
/// [2]       -> message.values[2]
/// [2, 4]    -> message.values[2].record[4]
/// [2, 4, 1] -> message.values[2].record[4].record[1]
fn get_message_value<'a>(message: &'a Message, path: &[usize]) -> Option<&'a Value> {
    if path.is_empty() {
        return None;
    }

    let first = path[0];
    let mut value = message.values_slice().get(first)?;

    for &index in &path[1..] {
        value = get_record_value(value, index)?;
    }

    Some(value)
}

/// Mutable version of get_message_value.
fn get_message_value_mut<'a>(message: &'a mut Message, path: &[usize]) -> Option<&'a mut Value> {
    if path.is_empty() {
        return None;
    }

    let first = path[0];
    let mut value = message.values_slice_mut().get_mut(first)?;

    for &index in &path[1..] {
        value = get_record_value_mut(value, index)?;
    }

    Some(value)
}

fn get_record_value<'a>(value: &'a Value, index: usize) -> Option<&'a Value> {
    if value.kind != ValueKind::ValueRecord {
        return None;
    }

    unsafe { value.data.record_.as_slice().get(index) }
}

fn get_record_value_mut<'a>(value: &'a mut Value, index: usize) -> Option<&'a mut Value> {
    if value.kind != ValueKind::ValueRecord {
        return None;
    }

    unsafe {
        let record = value.data.record_;
        std::slice::from_raw_parts_mut(record.ptr as *mut Value, record.len).get_mut(index)
    }
}

/// Deep-clones a Value into the provided Arena.
///
/// Primitive values are copied directly.
///
/// String/Bytes/List/Record values allocate their backing storage
/// from the Arena.
unsafe fn clone_value(arena: &mut Arena, value: &Value) -> Value {
    match value.kind {
        ValueKind::ValueNone => Value::none(),
        ValueKind::ValueBool => unsafe { value.data.bool_ }.into(),
        ValueKind::ValueInt => unsafe { value.data.int_ }.into(),
        ValueKind::ValueUint => unsafe { value.data.uint_ }.into(),
        ValueKind::ValueFloat => unsafe { value.data.float_ }.into(),
        ValueKind::ValueString => unsafe { clone_str(arena, value.data.string_) }.into(),
        ValueKind::ValueBytes => unsafe { clone_slice(arena, value.data.bytes_) }.into(),
        ValueKind::ValueList => unsafe { clone_values(arena, value.data.list_) }.into(),
        ValueKind::ValueRecord => unsafe { clone_values(arena, value.data.record_) }.into(),
    }
}

unsafe fn clone_slice<T: Copy>(arena: &mut Arena, slice: sdk::Slice<T>) -> sdk::Slice<T> {
    if slice.len == 0 {
        return sdk::Slice::empty();
    }

    let dst = arena.alloc::<T>(slice.len);

    unsafe {
        std::ptr::copy_nonoverlapping(slice.ptr, dst, slice.len);
    }

    sdk::Slice::from_raw_parts(dst, slice.len)
}

unsafe fn clone_values(arena: &mut Arena, slice: sdk::Slice<Value>) -> sdk::Slice<Value> {
    if slice.len == 0 {
        return sdk::Slice::empty();
    }

    let src = unsafe { std::slice::from_raw_parts(slice.ptr, slice.len) };
    let dst = arena.alloc::<Value>(src.len());

    for (index, value) in src.iter().enumerate() {
        unsafe { dst.add(index).write(clone_value(arena, value)) };
    }

    sdk::Slice::from_raw_parts(dst, src.len())
}

unsafe fn clone_str(arena: &mut Arena, value: sdk::Str) -> sdk::Str {
    if value.len == 0 {
        return sdk::Str::empty();
    }

    let dst = arena.alloc::<u8>(value.len);

    unsafe {
        std::ptr::copy_nonoverlapping(value.ptr, dst, value.len);
    }

    sdk::Str {
        ptr: dst,
        len: value.len,
    }
}
