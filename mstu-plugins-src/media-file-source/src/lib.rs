use std::{path::PathBuf, ptr, vec};

use mstu_media::{
    self as media,
    Codec::{H264, H265},
};
use mstu_sdk::{
    CommandDescriptor, EventDescriptor, HostContext, Logger, Message, Metadata, PluginDescriptor,
    PluginHandle, ProcessContext, Schema, Slice, Str, Value, Writer, log_error, log_info, log_trace,
    log_warn, message, slice,
};

pub mod mp4;

pub mod file;
pub mod schemas;
pub mod types;

pub use file::Sample;

use crate::file::SampleReader;

static METADATA: Metadata = Metadata {
    name: Str::from_static("Media file source"),
    version: Str::from_static(env!("CARGO_PKG_VERSION")),
};

pub struct MediaFileSourcePlugin {
    id: String,
    ctx: *const HostContext,
    log: Logger,
    filename: String,
    sample_reader: Option<Box<dyn SampleReader>>,

    /// Samples read since the file was opened, and whether the end was
    /// already reported.
    samples: u64,
    ended: bool,
}

impl MediaFileSourcePlugin {
    fn new(ctx: *const HostContext, id: String) -> Self {
        let log = Logger::new(ctx, unsafe { METADATA.name.as_str() }, id.as_str());

        Self {
            ctx,
            log,
            id,
            filename: String::new(),
            sample_reader: None,
            samples: 0,
            ended: false,
        }
    }

    fn open(&mut self, path: PathBuf) -> bool {
        self.filename = path.to_string_lossy().to_string();
        self.samples = 0;
        self.ended = false;

        if path.extension().and_then(|s| s.to_str()) == Some("mp4") {
            match mp4::open(path) {
                Ok(reader) => {
                    self.sample_reader = Some(Box::new(reader));
                }
                Err(err) => {
                    log_error!(self.log, "cannot open '{}': {err}", self.filename);
                    return false;
                }
            }
        } else {
            log_error!(self.log, "unsupported media file '{}'", self.filename);
            return false;
        }

        let mut codec = Str::empty();
        let mut extras: Vec<u8> = vec![];

        if let Some(sample_reader) = self.sample_reader.as_ref() {
            match sample_reader.codec() {
                H264(params) => {
                    codec = Str::from_static("h264");
                    for nalus in [&params.sps, &params.pps] {
                        for nalu in nalus {
                            extras.extend_from_slice(&[0, 0, 0, 1]);
                            extras.extend_from_slice(nalu.as_bytes());
                        }
                    }
                }
                H265(params) => {
                    codec = Str::from_static("h265");

                    for nalus in [&params.vps, &params.sps, &params.pps] {
                        for nalu in nalus {
                            extras.extend_from_slice(&[0, 0, 0, 1]);
                            extras.extend_from_slice(nalu.as_bytes());
                        }
                    }
                }
                _ => {
                    log_error!(self.log, "unsupported codec type");
                    return false;
                }
            }
        }

        log_info!(
            self.log,
            "opened '{}': codec {codec}, {} byte(s) of extras",
            self.filename,
            extras.len()
        );

        unsafe {
            let event_writer =
                &mut *(((*self.ctx).new_event)(Str::new(self.id.as_str()), 0) as *mut Writer);

            (event_writer.set_str)(event_writer, 0, Str::new(self.filename.as_str()));
            (event_writer.set_str)(event_writer, 1, codec);
            (event_writer.set_bytes)(event_writer, 2, Slice::from(extras.as_slice()));

            ((*self.ctx).publish_events)(Str::new(self.id.as_str()), 0);
        }

        log_info!(self.log, "published file.changed for '{}'", self.filename);

        true
    }

    fn process(&mut self, output: &mut message::Writer) -> Result<(), String> {
        // A source with no file yet produces nothing, it is not an error: the
        // worker asks again every few milliseconds until one is set.
        let Some(sample_reader) = self.sample_reader.as_mut() else {
            return Ok(());
        };

        let sample = match sample_reader.next_sample()? {
            Some(sample) => sample,
            None => {
                // The source keeps spinning at the end of the file, so say so
                // once instead of every round.
                if !self.ended {
                    self.ended = true;

                    log_info!(
                        self.log,
                        "end of '{}' after {} sample(s)",
                        self.filename,
                        self.samples
                    );
                }

                return Ok(());
            }
        };

        self.samples += 1;

        log_trace!(self.log, "sample {}: {}", self.samples, sample);

        (output.set_uint)(output, 0, sample.start_time);
        (output.set_uint)(output, 1, sample.duration as u64);
        (output.set_int)(output, 2, sample.rendering_offset as i64);
        (output.set_bool)(output, 3, sample.is_sync);
        (output.set_bytes)(
            output,
            4,
            Slice::from_raw_parts(sample.bytes.as_ptr(), sample.bytes.len()),
        );

        Ok(())
    }
}

extern "C" fn metadata() -> &'static Metadata {
    &METADATA
}

extern "C" fn create(ctx: *const HostContext, id: Str) -> PluginHandle {
    let plugin = MediaFileSourcePlugin::new(ctx, id.to_string());

    log_info!(plugin.log, "created");

    Box::into_raw(Box::new(plugin)) as PluginHandle
}

extern "C" fn id(instance: PluginHandle) -> Str {
    let plugin = unsafe { &*(instance as *mut MediaFileSourcePlugin) };
    Str::new(plugin.id.as_str())
}

extern "C" fn release(instance: PluginHandle) {
    unsafe {
        let plugin = Box::from_raw(instance as *mut MediaFileSourcePlugin);

        log_info!(plugin.log, "released after {} sample(s)", plugin.samples);

        drop(plugin);
    }
}

extern "C" fn process(instance: PluginHandle, p_ctx: *mut ProcessContext) -> bool {
    let plugin = unsafe { &mut *(instance as *mut MediaFileSourcePlugin) };

    match plugin.process(unsafe { &mut *(*p_ctx).output }) {
        Ok(()) => true,
        Err(err) => {
            log_error!(plugin.log, "{err}");
            false
        }
    }
}

extern "C" fn process_input_schema() -> *const Schema {
    ptr::null()
}

extern "C" fn process_output_schema() -> *const Schema {
    &schemas::VIDEO_OUTPUT_SCHEMA
}

extern "C" fn settings_schema() -> *const Schema {
    &schemas::SETTINGS_SCHEMA
}

extern "C" fn settings(_instance: PluginHandle, _output: *mut message::Writer) -> bool {
    true
}

extern "C" fn set_parameter(instance: PluginHandle, field: usize, value: Value) -> bool {
    let plugin = unsafe { &mut *(instance as *mut MediaFileSourcePlugin) };

    match field {
        0 => {
            let Some(path) = value.get::<Str>() else {
                log_error!(plugin.log, "parameter 0 (path) must be a string");
                return false;
            };

            unsafe { plugin.open(PathBuf::from(path.as_str())) }
        }
        _ => {
            log_warn!(plugin.log, "ignored unknown parameter {field}");
            true
        }
    }
}

/// UI folder, resolved by the host against the plugin library directory.
extern "C" fn ui() -> Str {
    Str::from_static("media-file-source-ui")
}

extern "C" fn events() -> Slice<EventDescriptor> {
    slice!(schemas::EVENTS)
}

extern "C" fn commands() -> Slice<CommandDescriptor> {
    Slice::empty()
}

extern "C" fn invoke(_instance: PluginHandle, _command: usize, _input: *const Message) -> bool {
    true
}

static PLUGIN: PluginDescriptor = PluginDescriptor {
    metadata,
    create,
    id,
    release,

    process_input_schema,
    process_output_schema,
    settings_schema,

    settings,
    set_parameter,

    ui,

    process,

    events,
    commands,
    invoke,
};

#[unsafe(no_mangle)]
pub extern "C" fn plugin_descriptor() -> *const PluginDescriptor {
    &PLUGIN
}
