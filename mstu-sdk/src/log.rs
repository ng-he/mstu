use std::fmt;

use crate::{HostContext, Str};

#[repr(C)]
#[derive(Clone, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

pub type LogFn = extern "C" fn(level: LogLevel, target: Str, message: Str);

/// Sends a plugin's messages to the host logger under a stable target.
///
/// Holds the borrowed `HostContext`, so it must not outlive the plugin.
pub struct Logger {
    ctx: *const HostContext,
    target: String,
}

impl Logger {
    /// Targets read as `Media dumper (a1b2c3)`, so instances stay apart.
    pub fn new(ctx: *const HostContext, name: &str, id: &str) -> Self {
        Self {
            ctx,
            target: format!("{name} ({id})"),
        }
    }

    /// False when the host would drop the message, so the caller can skip
    /// formatting it.
    pub fn enabled(&self, level: LogLevel) -> bool {
        unsafe { ((*self.ctx).log_enabled)(level) }
    }

    pub fn log(&self, level: LogLevel, message: fmt::Arguments) {
        let message = message.to_string();

        unsafe {
            ((*self.ctx).logger)(
                level,
                Str::new(self.target.as_str()),
                Str::new(message.as_str()),
            );
        }
    }
}

/// `log!(logger, Info, "opened {path}")`, and one macro per level below.
#[macro_export]
macro_rules! log {
    ($logger:expr, $level:ident, $($arg:tt)*) => {
        if $logger.enabled($crate::LogLevel::$level) {
            $logger.log($crate::LogLevel::$level, format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! log_trace {
    ($logger:expr, $($arg:tt)*) => { $crate::log!($logger, Trace, $($arg)*) };
}

#[macro_export]
macro_rules! log_debug {
    ($logger:expr, $($arg:tt)*) => { $crate::log!($logger, Debug, $($arg)*) };
}

#[macro_export]
macro_rules! log_info {
    ($logger:expr, $($arg:tt)*) => { $crate::log!($logger, Info, $($arg)*) };
}

#[macro_export]
macro_rules! log_warn {
    ($logger:expr, $($arg:tt)*) => { $crate::log!($logger, Warn, $($arg)*) };
}

#[macro_export]
macro_rules! log_error {
    ($logger:expr, $($arg:tt)*) => { $crate::log!($logger, Error, $($arg)*) };
}
