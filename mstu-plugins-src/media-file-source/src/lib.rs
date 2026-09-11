use std::{
    path::PathBuf, ptr,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
    vec,
};

use mstu_media::{
    self as media,
    Codec::{H264, H265},
};
use mstu_sdk::{
    CommandDescriptor, EventDescriptor, HostContext, Logger, Message, Metadata, PluginDescriptor,
    PluginHandle, ProcessContext, Schema, Slice, Str, Value, Writer, log_debug, log_error, log_info,
    log_trace, log_warn, message, slice,
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
    /// already reported. Atomic: the host polls them while `process` runs.
    samples: AtomicU64,
    ended: AtomicBool,

    /// Where playback has reached and how long the file is, in microseconds.
    position: AtomicU64,
    duration: AtomicU64,

    /// Playback speed: 1.0 real time, 0 as fast as the pipeline will take it.
    rate: f64,
    looping: bool,

    /// Wall clock and media time that pacing is measured from.
    anchor: Option<Instant>,
    anchor_time: u64,

    /// Read but not yet due, held back until its turn comes.
    pending: Option<Sample>,
}

/// Pacing resolution is bounded by the engine's idle poll, so a sample this
/// far behind means the clock jumped: a pause, a seek, or a slow consumer.
const MAX_LAG: Duration = Duration::from_millis(500);

impl MediaFileSourcePlugin {
    fn new(ctx: *const HostContext, id: String) -> Self {
        let log = Logger::new(ctx, unsafe { METADATA.name.as_str() }, id.as_str());

        Self {
            ctx,
            log,
            id,
            filename: String::new(),
            sample_reader: None,
            samples: AtomicU64::new(0),
            ended: AtomicBool::new(false),
            position: AtomicU64::new(0),
            duration: AtomicU64::new(0),
            rate: 1.0,
            looping: false,
            anchor: None,
            anchor_time: 0,
            pending: None,
        }
    }

    /// Media time in timescale units to microseconds.
    fn micros(&self, time: u64) -> u64 {
        match self.sample_reader.as_ref().map(|reader| reader.timescale()) {
            Some(timescale) if timescale > 0 => time * 1_000_000 / timescale as u64,
            _ => 0,
        }
    }

    /// Restarts the clock from `time`, so pacing resumes rather than bursting
    /// to catch up.
    fn anchor_at(&mut self, time: u64) {
        self.anchor = Some(Instant::now());
        self.anchor_time = time;
    }

    /// When a sample is due, measured from the anchor.
    fn due(&self, time: u64) -> Option<Instant> {
        let anchor = self.anchor?;

        if self.rate <= 0.0 {
            return Some(anchor);
        }

        let timescale = self.sample_reader.as_ref()?.timescale().max(1) as f64;
        let ahead = time.saturating_sub(self.anchor_time) as f64 / timescale / self.rate;

        Some(anchor + Duration::from_secs_f64(ahead))
    }

    fn seek_to(&mut self, micros: u64) {
        let Some(reader) = self.sample_reader.as_mut() else {
            return;
        };

        let time = micros * reader.timescale() as u64 / 1_000_000;

        if !reader.seek(time) {
            log_warn!(self.log, "this source cannot seek");
            return;
        }

        self.pending = None;
        self.ended.store(false, Ordering::Relaxed);
        self.position.store(micros, Ordering::Relaxed);
        self.anchor_at(time);

        log_info!(self.log, "seeked to {micros} us");
    }

    fn open(&mut self, path: PathBuf) -> bool {
        self.filename = path.to_string_lossy().to_string();
        self.samples.store(0, Ordering::Relaxed);
        self.ended.store(false, Ordering::Relaxed);
        self.position.store(0, Ordering::Relaxed);
        self.anchor = None;
        self.pending = None;

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

        let duration = self
            .sample_reader
            .as_ref()
            .and_then(|reader| reader.duration())
            .unwrap_or(0);

        self.duration
            .store(self.micros(duration), Ordering::Relaxed);

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

        // A sample read last round that was not due yet.
        let sample = match self.pending.take() {
            Some(sample) => sample,
            None => match sample_reader.next_sample()? {
                Some(sample) => sample,
                None => return self.finish(),
            },
        };

        // Hold the sample back until its place in the timeline comes round.
        if self.rate > 0.0 {
            if self.anchor.is_none() {
                self.anchor_at(sample.start_time);
            }

            if let Some(due) = self.due(sample.start_time) {
                let now = Instant::now();

                if now < due {
                    self.pending = Some(sample);
                    return Ok(());
                }

                // Being this late means the clock jumped rather than drifted,
                // so start counting again from here.
                if now.duration_since(due) > MAX_LAG {
                    self.anchor_at(sample.start_time);
                }
            }
        }

        let samples = self.samples.fetch_add(1, Ordering::Relaxed) + 1;
        self.position
            .store(self.micros(sample.start_time), Ordering::Relaxed);

        log_trace!(self.log, "sample {samples}: {sample}");

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

    /// End of the file: start over, or say so once and go quiet.
    fn finish(&mut self) -> Result<(), String> {
        if self.looping {
            if let Some(reader) = self.sample_reader.as_mut() {
                reader.rewind()?;
            }

            self.anchor = None;
            self.position.store(0, Ordering::Relaxed);

            log_debug!(self.log, "looping '{}'", self.filename);

            return Ok(());
        }

        // The source keeps spinning at the end of the file, so say so once
        // instead of every round.
        if !self.ended.swap(true, Ordering::Relaxed) {
            log_info!(
                self.log,
                "end of '{}' after {} sample(s)",
                self.filename,
                self.samples.load(Ordering::Relaxed)
            );

            // file.ended carries no payload, but it still has to be queued
            // before it can be published.
            unsafe {
                ((*self.ctx).new_event)(Str::new(self.id.as_str()), 1);
                ((*self.ctx).publish_events)(Str::new(self.id.as_str()), 1);
            }

            log_info!(self.log, "published file.ended");
        }

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

        log_info!(plugin.log, "released after {} sample(s)", plugin.samples.load(Ordering::Relaxed));

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

        // Changing speed re-anchors, otherwise the new rate is measured from
        // the old start and the reader bursts or stalls to catch up.
        1 => {
            plugin.rate = value.get::<f64>().unwrap_or(1.0).max(0.0);
            plugin.anchor = None;

            log_info!(plugin.log, "rate {}", plugin.rate);
            true
        }

        2 => {
            plugin.looping = value.get::<bool>().unwrap_or(false);

            log_info!(plugin.log, "loop {}", plugin.looping);
            true
        }

        3 => {
            plugin.seek_to(value.get::<u64>().unwrap_or(0));
            true
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

extern "C" fn live_schema() -> *const Schema {
    &schemas::LIVE_SCHEMA
}

/// Polled by the host while `process` runs, so it only reads atomics.
extern "C" fn live(instance: PluginHandle, output: *mut message::Writer) -> bool {
    let plugin = unsafe { &*(instance as *mut MediaFileSourcePlugin) };
    let output = unsafe { &mut *output };

    (output.set_uint)(output, 0, plugin.samples.load(Ordering::Relaxed));
    (output.set_bool)(output, 1, plugin.ended.load(Ordering::Relaxed));
    (output.set_uint)(output, 2, plugin.position.load(Ordering::Relaxed));
    (output.set_uint)(output, 3, plugin.duration.load(Ordering::Relaxed));

    true
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

    live_schema,
    live,
};

#[unsafe(no_mangle)]
pub extern "C" fn plugin_descriptor() -> *const PluginDescriptor {
    &PLUGIN
}
