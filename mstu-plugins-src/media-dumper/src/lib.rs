use std::{
    format,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    ptr,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use mstu_sdk::{
    CommandDescriptor, EventDescriptor, HostContext, Logger, Message, Metadata, PluginDescriptor,
    PluginHandle, ProcessContext, Schema, Slice, Str, Value, log_debug, log_error, log_info,
    log_trace, log_warn, message, slice,
};

use crate::schemas::{COMMANDS, INPUT_SCHEMA};

pub mod schemas;

static METADATA: Metadata = Metadata {
    name: Str::from_static("Media dumper"),
    version: Str::from_static(env!("CARGO_PKG_VERSION")),
};

pub struct MediaDumperPlugin {
    id: String,
    ctx: *const HostContext,
    log: Logger,
    output_dir: String,
    current_filename: String,
    /// Locked: a `file.ended` subscription finishes the file from the source's
    /// worker thread while this plugin's own worker may be writing.
    output: Mutex<Option<BufWriter<File>>>,

    /// Written to the current file, reported on finish. Atomic: the host polls
    /// them while `process` runs.
    chunks: AtomicU64,
    bytes: AtomicU64,
}

impl MediaDumperPlugin {
    fn new(ctx: *const HostContext, id: String) -> Self {
        let log = Logger::new(ctx, unsafe { METADATA.name.as_str() }, id.as_str());

        Self {
            ctx,
            log,
            id,
            output_dir: String::new(),
            current_filename: String::new(),
            output: Mutex::new(None),
            chunks: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
        }
    }

    /// Pushes what the UI shows; the host keeps the latest.
    fn publish_live(&self) {
        let name = match self.output.lock().unwrap().is_some() {
            true => Str::new(self.current_filename.as_str()),
            false => Str::from_static(""),
        };

        let values = [
            name.into(),
            self.chunks.load(Ordering::Relaxed).into(),
            self.bytes.load(Ordering::Relaxed).into(),
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

    fn change_codec(&mut self, codec: &str, extras: &[u8]) -> bool {
        self.finish();

        let ext = match codec {
            "h264" => "h264",
            "h265" => "h265",
            _ => {
                log_error!(self.log, "unsupported codec '{codec}'");
                return false;
            }
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // The id keeps dumpers in the same pipeline from sharing a file.
        let filename = format!("dump_{}_{}.{}", timestamp, self.id, ext);
        self.current_filename = filename.clone();
        let path = PathBuf::from(self.output_dir.clone()).join(filename);

        let file = match File::create(&path) {
            Ok(file) => file,
            Err(err) => {
                log_error!(self.log, "cannot create '{}': {err}", path.display());
                return false;
            }
        };

        log_info!(self.log, "writing {codec} to '{}'", path.display());

        *self.output.lock().unwrap() = Some(BufWriter::new(file));
        self.chunks.store(0, Ordering::Relaxed);
        self.bytes.store(0, Ordering::Relaxed);

        self.write(extras)
    }

    fn write(&mut self, data: &[u8]) -> bool {
        {
            let mut output = self.output.lock().unwrap();

            let Some(writer) = output.as_mut() else {
                log_warn!(self.log, "dropped {} byte(s), no file open", data.len());
                return true;
            };

            if let Err(e) = writer.write_all(data) {
                log_error!(self.log, "{e}");
                return false;
            }
        }

        let chunks = self.chunks.fetch_add(1, Ordering::Relaxed) + 1;
        let bytes = self.bytes.fetch_add(data.len() as u64, Ordering::Relaxed) + data.len() as u64;

        log_trace!(self.log, "wrote {} byte(s)", data.len());

        // A per-write log is far too loud, so mark progress now and then.
        if chunks % 1000 == 0 {
            log_debug!(
                self.log,
                "{chunks} chunk(s), {bytes} byte(s) into '{}'",
                self.current_filename
            );
        }

        self.publish_live();

        true
    }

    fn finish(&mut self) -> bool {
        let taken = self.output.lock().unwrap().take();

        if let Some(mut writer) = taken {
            if let Err(e) = writer.flush() {
                log_error!(self.log, "{e}");
                return false;
            }

            log_info!(
                self.log,
                "finished {}/{}: {} chunk(s), {} byte(s)",
                self.output_dir,
                self.current_filename,
                self.chunks.load(Ordering::Relaxed),
                self.bytes.load(Ordering::Relaxed)
            );
        }

        self.publish_live();

        true
    }

    fn set_output_path(&mut self, dir: String) {
        log_info!(self.log, "output directory is '{dir}'");

        self.output_dir = dir
    }
}

extern "C" fn metadata() -> &'static Metadata {
    &METADATA
}

extern "C" fn create(ctx: *const HostContext, id: Str) -> PluginHandle {
    let plugin = MediaDumperPlugin::new(ctx, id.to_string());

    log_info!(plugin.log, "created");

    // So a page opening before anything happens has readings to show.
    plugin.publish_live();

    Box::into_raw(Box::new(plugin)) as PluginHandle
}

extern "C" fn id(instance: PluginHandle) -> Str {
    let plugin = unsafe { &*(instance as *mut MediaDumperPlugin) };
    Str::new(plugin.id.as_str())
}

extern "C" fn release(instance: PluginHandle) {
    unsafe {
        let mut plugin = Box::from_raw(instance as *mut MediaDumperPlugin);

        plugin.finish();

        log_info!(plugin.log, "released");

        drop(plugin);
    }
}

extern "C" fn process(instance: PluginHandle, p_ctx: *mut ProcessContext) -> bool {
    let plugin = unsafe { &mut *(instance as *mut MediaDumperPlugin) };

    // The engine checks the input against the schema, so field 0 is bytes.
    let field = unsafe { (*(*p_ctx).input).values.at(0).get::<Slice<u8>>() };

    let Some(data) = field else {
        log_error!(plugin.log, "input field 0 is not bytes");
        return false;
    };

    plugin.write(unsafe { data.as_slice() })
}

extern "C" fn process_input_schema() -> *const Schema {
    &INPUT_SCHEMA
}

extern "C" fn process_output_schema() -> *const Schema {
    ptr::null()
}

extern "C" fn settings_schema() -> *const Schema {
    &schemas::SETTINGS_SCHEMA
}

extern "C" fn settings(_instance: PluginHandle, _output: *mut message::Writer) -> bool {
    true
}

extern "C" fn set_parameter(instance: PluginHandle, field: usize, value: Value) -> bool {
    let plugin = unsafe { &mut *(instance as *mut MediaDumperPlugin) };

    match field {
        0 => unsafe {
            let output_dir: Str = value.get().unwrap_or(Str::from_static(""));
            plugin.set_output_path(String::from(output_dir.as_str()));
            true
        },
        _ => {
            log_warn!(plugin.log, "ignored unknown parameter {field}");
            true
        }
    }
}

/// UI folder, resolved by the host against the plugin library directory.
extern "C" fn ui() -> Str {
    Str::from_static("media-dumper-ui")
}

extern "C" fn live_schema() -> *const Schema {
    &schemas::LIVE_SCHEMA
}

extern "C" fn events() -> Slice<EventDescriptor> {
    Slice::empty()
}

extern "C" fn commands() -> Slice<CommandDescriptor> {
    slice!(COMMANDS)
}

extern "C" fn invoke(instance: PluginHandle, command: usize, input: *const Message) -> bool {
    let plugin = unsafe { &mut *(instance as *mut MediaDumperPlugin) };
    let values = unsafe { &(*input).values };

    log_debug!(plugin.log, "command {command} with {:?}", unsafe { &*input });

    match command {
        0 => {
            // The engine checks the payload against the command schema.
            let (Some(codec), Some(extras)) =
                (values.at(0).get::<Str>(), values.at(1).get::<Slice<u8>>())
            else {
                log_error!(plugin.log, "file.changed payload is not (string, bytes)");
                return false;
            };

            unsafe { plugin.change_codec(codec.as_str(), extras.as_slice()) }
        }
        1 => plugin.finish(),
        _ => {
            log_warn!(plugin.log, "ignored unknown command {command}");
            true
        }
    }
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
