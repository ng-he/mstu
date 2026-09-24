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
        }
    }

    /// Pushes what the UI shows; the host keeps the latest.
    fn publish_live(&self) {
        let values = [
            self.samples.load(Ordering::Relaxed).into(),
            self.ended.load(Ordering::Relaxed).into(),
            self.position.load(Ordering::Relaxed).into(),
            self.duration.load(Ordering::Relaxed).into(),
        ];

        unsafe {
            ((*self.ctx).publish_live)(
                Str::new(self.id.as_str()),
                Message {
                    values: Slice::from_raw_parts(values.as_ptr(), values.len()),
                },
            );
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

        self.ended.store(false, Ordering::Relaxed);
        self.position.store(micros, Ordering::Relaxed);
        self.anchor_at(time);
        self.publish_live();

        log_info!(self.log, "seeked to {micros} us");
    }

    fn open(&mut self, path: PathBuf) -> bool {
        self.filename = path.to_string_lossy().to_string();
        self.samples.store(0, Ordering::Relaxed);
        self.ended.store(false, Ordering::Relaxed);
        self.position.store(0, Ordering::Relaxed);
        self.anchor = None;

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

            let values = [
                Str::new(self.filename.as_str()).into(),
                codec.into(),
                Slice::from(extras.as_slice()).into(),
            ];

            (event_writer.fill)(
                event_writer,
                Message {
                    values: Slice::from_raw_parts(values.as_ptr(), values.len()),
                },
            );

            ((*self.ctx).publish_events)(Str::new(self.id.as_str()), 0);
        }

        log_info!(self.log, "published file.changed for '{}'", self.filename);

        self.publish_live();

        true
    }

    fn process(&mut self, output: &mut message::Writer) -> Result<(), String> {
        // A source with no file yet produces nothing, it is not an error: the
        // worker asks again every few milliseconds until one is set.
        let Some(reader) = self.sample_reader.as_mut() else {
            return Ok(());
        };

        // Described before it is read, so nothing is read before it is due.
        let Some(sample) = reader.peek_sample()? else {
            return self.finish();
        };

        // Hold the sample back until its place in the timeline comes round.
        if self.rate > 0.0 {
            if self.anchor.is_none() {
                self.anchor_at(sample.start_time);
            }

            if let Some(due) = self.due(sample.start_time) {
                let now = Instant::now();

                if now < due {
                    return Ok(());
                }

                // Being this late means the clock jumped rather than drifted,
                // so start counting again from here.
                if now.duration_since(due) > MAX_LAG {
                    self.anchor_at(sample.start_time);
                }
            }
        }

        // Read straight into the message: the plugin never holds the payload.
        let payload = (output.reserve)(output, sample.size);

        if payload.is_null() {
            return Err("the message would not take the sample".to_string());
        }

        let Some(reader) = self.sample_reader.as_mut() else {
            return Ok(());
        };

        reader.read_sample(unsafe { std::slice::from_raw_parts_mut(payload, sample.size) })?;

        let bytes = Slice::from_raw_parts(payload, sample.size);
        let samples = self.samples.fetch_add(1, Ordering::Relaxed) + 1;

        self.position
            .store(self.micros(sample.start_time), Ordering::Relaxed);

        log_trace!(self.log, "sample {samples}: {sample}");

        let values = [
            sample.start_time.into(),
            (sample.duration as u64).into(),
            (sample.rendering_offset as i64).into(),
            sample.is_sync.into(),
            bytes.into(),
        ];

        (output.fill)(
            output,
            Message {
                values: Slice::from_raw_parts(values.as_ptr(), values.len()),
            },
        );

        self.publish_live();

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
            self.publish_live();

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

            self.publish_live();
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

    // So a page opening before anything happens has readings to show.
    plugin.publish_live();

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
};

#[unsafe(no_mangle)]
pub extern "C" fn plugin_descriptor() -> *const PluginDescriptor {
    &PLUGIN
}
