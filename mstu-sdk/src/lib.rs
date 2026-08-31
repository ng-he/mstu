pub mod context;
pub mod log;
pub mod message;
pub mod plugin;
pub mod schema;
pub mod types;
pub mod value;

pub use context::*;
pub use log::*;
pub use message::*;
pub use plugin::*;
pub use schema::*;
pub use types::*;
pub use value::*;

#[unsafe(no_mangle)]
pub extern "C" fn value_get(slice: Slice<Value>, index: usize) -> *const Value {
    if index >= slice.len {
        return std::ptr::null();
    }

    unsafe { slice.ptr.add(index) }
}
