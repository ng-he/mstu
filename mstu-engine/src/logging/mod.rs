use std::fmt;

use crate::sdk::{LogLevel, Str};
use time::UtcOffset;
use time::macros::format_description;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::fmt::time::OffsetTime;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Per-message logs are trace, per-node ones debug. `RUST_LOG=debug` or
/// `RUST_LOG=mstu_engine=trace` turns them on, info is the default.
pub fn init() {
    let offset = UtcOffset::current_local_offset().expect("should get local offset!");
    let timer = OffsetTime::new(
        offset,
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"),
    );

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_level(true)
        .with_timer(timer.clone())
        .with_filter(filter);

    tracing_subscriber::registry()
        .with(console_layer)
        .try_init()
        .ok();
}

pub extern "C" fn log_impl(level: LogLevel, target: Str, message: Str) {
    match level {
        LogLevel::Trace => tracing::trace!("[{target}] {message}"),
        LogLevel::Debug => tracing::debug!("[{target}] {message}"),
        LogLevel::Info => tracing::info!("[{target}] {message}"),
        LogLevel::Warn => tracing::warn!("[{target}] {message}"),
        LogLevel::Error => tracing::error!("[{target}] {message}"),
    }
}

fn level_of(level: LogLevel) -> tracing::Level {
    match level {
        LogLevel::Trace => tracing::Level::TRACE,
        LogLevel::Debug => tracing::Level::DEBUG,
        LogLevel::Info => tracing::Level::INFO,
        LogLevel::Warn => tracing::Level::WARN,
        LogLevel::Error => tracing::Level::ERROR,
    }
}

/// Lets a caller skip formatting a message the subscriber would drop.
///
/// `HostContext::log_enabled`, so a plugin logging per sample costs nothing
/// while trace is off.
pub extern "C" fn enabled(level: LogLevel) -> bool {
    LevelFilter::current() >= level_of(level)
}

/// `module_path!()` without the crate name: `runtime::pipeline`.
pub fn target(module: &str) -> &str {
    let target = module
        .strip_prefix(env!("CARGO_CRATE_NAME"))
        .unwrap_or(module)
        .trim_start_matches("::");

    if target.is_empty() { "engine" } else { target }
}

/// Engine side of the logger plugins use, so both land in the same stream.
pub fn log(level: LogLevel, target: &str, message: fmt::Arguments) {
    let message = message.to_string();

    log_impl(level, Str::new(target), Str::new(message.as_str()));
}

/// `log_at!(Info, "started {name}")`, targeted at the calling module.
#[macro_export]
macro_rules! log_at {
    ($level:ident, $($arg:tt)*) => {
        if $crate::logging::enabled($crate::sdk::LogLevel::$level) {
            $crate::logging::log(
                $crate::sdk::LogLevel::$level,
                $crate::logging::target(module_path!()),
                format_args!($($arg)*),
            );
        }
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => { $crate::log_at!(Trace, $($arg)*) };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => { $crate::log_at!(Debug, $($arg)*) };
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => { $crate::log_at!(Info, $($arg)*) };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => { $crate::log_at!(Warn, $($arg)*) };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => { $crate::log_at!(Error, $($arg)*) };
}
