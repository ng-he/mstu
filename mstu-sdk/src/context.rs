use crate::{LogFn, LogLevel, Message, Str, Value, message};

/// Dummy function to force `Value` to be emitted before dependent types in the
/// generated C header. This works around a cbindgen declaration-ordering issue.
#[doc(hidden)]
#[unsafe(no_mangle)]
pub extern "C" fn __cbindgen_force_value(_v: Value) {}

#[repr(C)]
pub struct HostContext {
    /// Host logger.
    pub logger: LogFn,

    /// Whether the host would record a message at `level`.
    ///
    /// Lets a plugin skip formatting a message that would be dropped, so a
    /// per-sample trace log costs nothing while trace is off.
    pub log_enabled: extern "C" fn(level: LogLevel) -> bool,

    /// Creates a new pending event for the given plugin and event type.
    ///
    /// Returns a writer used to fill the event payload. The event remains
    /// pending until `publish_event` is called.
    pub new_event: extern "C" fn(plugin_id: Str, event: usize) -> *const message::Writer,

    /// Publishes all pending events for the given plugin and event type.
    ///
    /// Pending events are published as a single batch and their temporary
    /// resources may be reclaimed afterwards.
    pub publish_events: extern "C" fn(plugin_id: Str, event: usize),

    /// Publishes the plugin's live values, one per field of `live_schema`.
    ///
    /// The host copies what it needs before returning, so the snapshot may
    /// point at anything the plugin owns. The latest one wins, so publishing
    /// often is cheap: the host sends the UI what moved, when it moved.
    pub publish_live: extern "C" fn(plugin_id: Str, snapshot: Message),
}

#[repr(C)]
pub struct ProcessContext {
    pub input: *const Message,
    pub output: *mut message::Writer,
}
