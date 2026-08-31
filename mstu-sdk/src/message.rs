use std::{ffi::c_void, fmt, write, writeln};

use crate::{Slice, Str, Value};

#[repr(C)]
pub struct Message {
    pub values: Slice<Value>,
}

impl Message {
    pub fn values_slice(&self) -> &[Value] {
        unsafe { self.values.as_slice() }
    }

    pub fn values_slice_mut(&mut self) -> &mut [Value] {
        // An empty message is a null pointer, which from_raw_parts rejects.
        if self.values.ptr.is_null() || self.values.len == 0 {
            return &mut [];
        }

        unsafe { std::slice::from_raw_parts_mut(self.values.ptr as *mut Value, self.values.len) }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let values = unsafe { self.values.as_slice() };

        writeln!(f, "Message {{")?;

        for (i, value) in values.iter().enumerate() {
            writeln!(f, "  [{i}] = {value}")?;
        }

        write!(f, "}}")
    }
}

/// One line, for logs. `Display` stays multi-line for the console.
impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let values = unsafe { self.values.as_slice() };

        write!(f, "[")?;

        for (i, value) in values.iter().enumerate() {
            if i != 0 {
                write!(f, ", ")?;
            }

            write!(f, "{value}")?;
        }

        write!(f, "]")
    }
}

#[repr(C)]
pub struct Writer {
    /// Reserved for engine use.
    /// Plugins must never read or modify this field.
    pub _engine_data: *mut c_void,

    pub set_none: extern "C" fn(w: *mut Writer, field: usize) -> bool,
    pub set_bool: extern "C" fn(w: *mut Writer, field: usize, value: bool) -> bool,
    pub set_int: extern "C" fn(w: *mut Writer, field: usize, value: i64) -> bool,
    pub set_uint: extern "C" fn(w: *mut Writer, field: usize, value: u64) -> bool,
    pub set_float: extern "C" fn(w: *mut Writer, field: usize, value: f64) -> bool,
    pub set_str: extern "C" fn(w: *mut Writer, field: usize, value: Str) -> bool,
    pub set_bytes: extern "C" fn(w: *mut Writer, field: usize, value: Slice<u8>) -> bool,
    pub set_record: extern "C" fn(w: *mut Writer, field: usize, value: Slice<Value>) -> bool,
    pub set_list: extern "C" fn(w: *mut Writer, field: usize, value: Slice<Value>) -> bool,
}
