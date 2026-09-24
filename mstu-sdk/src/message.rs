use std::{ffi::c_void, fmt, write, writeln};

use crate::{Slice, Value};

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

/// Fills in a message. The engine owns every byte one carries: a plugin
/// either lets `fill` copy what it has, or writes into `reserve`d memory.
#[repr(C)]
pub struct Writer {
    /// Reserved for engine use.
    /// Plugins must never read or modify this field.
    pub _engine_data: *mut c_void,

    /// Writes the whole message, one value per field, in order.
    ///
    /// What the values point at is copied in, except memory from `reserve`,
    /// which the message already owns. False if the count is wrong.
    pub fill: extern "C" fn(w: *mut Writer, message: Message) -> bool,

    /// Engine memory for `len` bytes, living as long as the message.
    ///
    /// A value handed to `fill` may point at it, and nothing is copied.
    pub reserve: extern "C" fn(w: *mut Writer, len: usize) -> *mut u8,
}

impl Writer {
    /// One value per field, in order.
    #[inline]
    pub fn fill(&mut self, values: &[Value]) -> bool {
        let message = Message {
            values: Slice::from_raw_parts(values.as_ptr(), values.len()),
        };

        (self.fill)(self, message)
    }

    /// Engine memory to write a payload into, with no copy afterwards.
    #[inline]
    pub fn reserve(&mut self, len: usize) -> Option<&mut [u8]> {
        let buffer = (self.reserve)(self, len);

        if buffer.is_null() {
            return None;
        }

        Some(unsafe { core::slice::from_raw_parts_mut(buffer, len) })
    }
}
