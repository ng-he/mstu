use crate::{HostContext, Message, ProcessContext, Schema, Slice, Str, Value, message};
use core::ffi::c_void;

pub type PluginHandle = *mut c_void;

#[repr(C)]
pub struct Metadata {
    pub name: Str,
    pub version: Str,
}

#[repr(C)]
pub struct CommandDescriptor {
    /// Command name.
    pub command_name: Str,

    /// Payload schema.
    pub schema: &'static Schema,

    // Description
    pub description: Str,
}

#[repr(C)]
pub struct EventDescriptor {
    /// Event name.
    pub event_name: Str,

    /// Payload schema.
    pub schema: &'static Schema,

    // Description
    pub description: Str,
}

#[repr(C)]
pub struct PluginDescriptor {
    /// Gets plugins metadata
    pub metadata: extern "C" fn() -> &'static Metadata,

    /// Creates a new plugin handle.
    pub create: extern "C" fn(ctx: *const HostContext, id: Str) -> PluginHandle,

    /// Get id of plugin
    pub id: extern "C" fn(handle: PluginHandle) -> Str,

    /// Releases/deallocate a plugin.
    pub release: extern "C" fn(handle: PluginHandle),

    /// Returns the process input schema.
    pub process_input_schema: extern "C" fn() -> *const Schema,

    /// Returns the process output schema.
    pub process_output_schema: extern "C" fn() -> *const Schema,

    /// Returns the settings schema.
    pub settings_schema: extern "C" fn() -> *const Schema,

    /// Gets the current settings.
    pub settings: extern "C" fn(handle: PluginHandle, output: *mut message::Writer) -> bool,

    /// Updates a single setting.
    pub set_parameter: extern "C" fn(handle: PluginHandle, field: usize, value: Value) -> bool,

    /// Returns the plugin UI entry.
    pub ui: extern "C" fn() -> Str,

    /// Process a stream.
    pub process: extern "C" fn(handle: PluginHandle, p_ctx: *mut ProcessContext) -> bool,

    /// Returns events.
    pub events: extern "C" fn() -> Slice<EventDescriptor>,

    /// Returns commands.
    pub commands: extern "C" fn() -> Slice<CommandDescriptor>,

    /// Invokes a command.
    pub invoke: extern "C" fn(handle: PluginHandle, command: usize, input: *const Message) -> bool,
}

unsafe extern "C" {
    /// Returns the plugin descriptor exported by the shared library.
    pub fn plugin_descriptor() -> *const PluginDescriptor;
}

pub type PluginDescriptorFn = unsafe extern "C" fn() -> *const PluginDescriptor;

unsafe impl Send for Slice<EventDescriptor> {}
unsafe impl Sync for Slice<EventDescriptor> {}

unsafe impl Send for Slice<CommandDescriptor> {}
unsafe impl Sync for Slice<CommandDescriptor> {}
