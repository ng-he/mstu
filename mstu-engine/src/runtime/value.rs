use mstu_sdk::{Slice, Str, Value, ValueKind};

use crate::runtime::alloc::Arena;

/// Deep-copies a value into `arena`.
///
/// Everything a message carries by reference — strings, bytes, lists and
/// records — is copied, because the value outlives whoever handed it over:
/// the plugin call that wrote it, or the message a mapping read it from.
///
/// # Safety
///
/// `value` must be a valid value whose buffers are still alive.
pub unsafe fn clone_value(arena: &mut Arena, value: &Value) -> Value {
    unsafe {
        match value.kind {
            ValueKind::ValueNone => Value::none(),

            ValueKind::ValueBool
            | ValueKind::ValueInt
            | ValueKind::ValueUint
            | ValueKind::ValueFloat => *value,

            ValueKind::ValueString => clone_str(arena, value.data.string_).into(),
            ValueKind::ValueBytes => clone_slice(arena, value.data.bytes_).into(),
            ValueKind::ValueList => Value::list(clone_values(arena, value.data.list_)),

            // Not `.into()`: that would hand back a list, since both carry the
            // same slice.
            ValueKind::ValueRecord => Value::record(clone_values(arena, value.data.record_)),
        }
    }
}

/// Takes a value a plugin wrote, copying only what the arena does not already
/// hold: a payload written into reserved memory is taken where it lies.
///
/// # Safety
///
/// `value` must be a valid value whose buffers are still alive.
pub unsafe fn take_value(arena: &mut Arena, value: &Value) -> Value {
    unsafe {
        match value.kind {
            ValueKind::ValueBytes if arena.owns(value.data.bytes_.ptr) => *value,
            ValueKind::ValueString if arena.owns(value.data.string_.ptr) => *value,
            _ => clone_value(arena, value),
        }
    }
}

/// # Safety
///
/// `slice` must be valid for `slice.len` elements, or empty.
pub unsafe fn clone_slice<T: Copy>(arena: &mut Arena, slice: Slice<T>) -> Slice<T> {
    if slice.ptr.is_null() || slice.len == 0 {
        return Slice::empty();
    }

    let dst = arena.alloc::<T>(slice.len);

    unsafe {
        std::ptr::copy_nonoverlapping(slice.ptr, dst, slice.len);
    }

    Slice::from_raw_parts(dst, slice.len)
}

/// # Safety
///
/// `slice` must be valid for `slice.len` values, or empty.
pub unsafe fn clone_values(arena: &mut Arena, slice: Slice<Value>) -> Slice<Value> {
    if slice.ptr.is_null() || slice.len == 0 {
        return Slice::empty();
    }

    let src = unsafe { slice.as_slice() };
    let dst = arena.alloc::<Value>(src.len());

    for (index, value) in src.iter().enumerate() {
        unsafe { dst.add(index).write(clone_value(arena, value)) };
    }

    Slice::from_raw_parts(dst, src.len())
}

/// # Safety
///
/// `value` must point at `value.len` bytes of UTF-8, or be empty.
pub unsafe fn clone_str(arena: &mut Arena, value: Str) -> Str {
    if value.ptr.is_null() || value.len == 0 {
        return Str::empty();
    }

    let dst = arena.alloc::<u8>(value.len);

    unsafe {
        std::ptr::copy_nonoverlapping(value.ptr, dst, value.len);
    }

    Str {
        ptr: dst,
        len: value.len,
    }
}
