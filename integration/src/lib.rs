//! Drives a real engine with the plugins that were collected beside it.
//!
//! Nothing here is mocked: the engine runs as its own process, the plugins are
//! the libraries in `plugins/`, and everything is said over the channel the
//! desktop app uses.

pub mod camera;
pub mod channel;

use core::panic;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use channel::Channel;

/// How long to wait for the engine to answer before calling it hung.
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

/// How long to wait for the engine to start answering at all.
const START_TIMEOUT: Duration = Duration::from_secs(20);

/// The repository this test lives in.
pub fn center() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("integration sits in the center repository")
        .to_path_buf()
}

/// The engine binary, release for preference since that is what is collected.
fn engine_binary() -> PathBuf {
    let name = format!("mstu-engine{}", std::env::consts::EXE_SUFFIX);

    let built = ["release", "debug"]
        .iter()
        .map(|profile| {
            center()
                .join("mstu-engine/target")
                .join(profile)
                .join(&name)
        })
        .find(|path| path.is_file());

    built.unwrap_or_else(|| {
        panic!(
            "no engine to test: build it with\n    \
             cargo build --release --manifest-path mstu-engine/Cargo.toml"
        )
    })
}

fn plugin_dir() -> PathBuf {
    let dir = center().join("plugins");

    let library = std::env::consts::DLL_EXTENSION;

    let collected = dir
        .read_dir()
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.path().extension().is_some_and(|x| x == library))
        })
        .unwrap_or(false);

    assert!(
        collected,
        "no plugins to test: build and collect them with\n    ./collect.sh --build"
    );

    dir
}

/// A directory nothing else is using, named after what wants it.
pub fn scratch(name: &str) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);

    let path = std::env::temp_dir().join(format!(
        "mstu-it-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));

    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("a directory to work in");

    path
}

/// An engine of its own: its own channel, its own run.
pub struct Engine {
    child: Child,
    runtime: PathBuf,
    pub address: String,
}

impl Engine {
    pub fn start(name: &str) -> Self {
        let runtime = scratch(name);
        let address = channel::address(&runtime);

        let child = Command::new(engine_binary())
            .arg("--serve")
            .env("MSTU_SOCKET", &address)
            .env("MSTU_PLUGIN_DIR", plugin_dir())
            .env("MSTU_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the engine to start");

        if let Err(error) = channel::wait_until_served(&address, START_TIMEOUT) {
            panic!("the engine never answered on {address}: {error}");
        }

        Self {
            child,
            runtime,
            address,
        }
    }

    pub fn connect(&self) -> Client {
        Client::connect(&self.address)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.runtime);
    }
}

/// A client of the engine, speaking the line protocol the desktop speaks.
pub struct Client {
    channel: Channel,
    next_id: u64,

    /// Everything the engine pushed while answers were being waited for.
    pub pushed: Vec<Value>,
}

impl Client {
    pub fn connect(address: &str) -> Self {
        let channel = Channel::try_connect(address).expect("the engine to be listening");

        Self {
            channel,
            next_id: 0,
            pushed: Vec::new(),
        }
    }

    /// Calls a method, and fails the test with what the engine said if it
    /// refused.
    pub fn call(&mut self, method: &str, params: Value) -> Value {
        self.try_call(method, params)
            .unwrap_or_else(|error| panic!("{method} failed: {error}"))
    }

    pub fn try_call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;

        let request = json!({ "id": id, "method": method, "params": params });

        self.channel.send(&request.to_string())?;

        loop {
            let message = self.read(REPLY_TIMEOUT)?;

            if message.get("id").and_then(Value::as_u64) != Some(id) {
                self.pushed.push(message);
                continue;
            }

            if message["ok"].as_bool() == Some(true) {
                return Ok(message["result"].clone());
            }

            return Err(message["error"]
                .as_str()
                .unwrap_or("no reason given")
                .to_string());
        }
    }

    fn read(&mut self, within: Duration) -> Result<Value, String> {
        let line = self.channel.line(within)?;

        serde_json::from_str(&line).map_err(|error| format!("{error}: {line}"))
    }

    /// Everything pushed since the last look, waiting out the given time so
    /// slower news still arrives.
    pub fn pushes(&mut self, within: Duration) -> Vec<Value> {
        let deadline = Instant::now() + within;

        while Instant::now() < deadline {
            match self.read(Duration::from_millis(120)) {
                Ok(message) => self.pushed.push(message),
                Err(_) => continue,
            }
        }

        std::mem::take(&mut self.pushed)
    }
}

// ---------- the pieces a pipeline is made of ----------

pub const SOURCE: &str = "mstu-media-file-source";
pub const DUMPER: &str = "mstu-media-dumper";
pub const RTSP: &str = "mstu-rtsp-source";
pub const IMAGES: &str = "mstu-image-decoder";

/// The source's payload field, and the dumper's.
pub const PAYLOAD: u64 = 4;
pub const DATA: u64 = 0;

impl Client {
    pub fn pipeline(&mut self, name: &str) -> u64 {
        self.call("create_pipeline", json!({ "name": name }))["pipeline"]
            .as_u64()
            .expect("a pipeline id")
    }

    pub fn plugin(&mut self, library: &str) -> String {
        self.call("create_plugin", json!({ "library": library }))["plugin"]
            .as_str()
            .expect("a plugin id")
            .to_string()
    }

    pub fn node(&mut self, pipeline: u64, plugin: &str) -> u64 {
        self.call(
            "add_node",
            json!({ "pipeline": pipeline, "plugin": plugin }),
        )["node"]
            .as_u64()
            .expect("a node id")
    }

    pub fn set(&mut self, plugin: &str, field: u64, value: Value) {
        self.call(
            "set_parameter",
            json!({ "plugin": plugin, "field": field, "value": value }),
        );
    }

    /// Wires several fields at once, which a node in the middle of a
    /// pipeline needs: it takes more than one of what a source hands over.
    pub fn connect_many(&mut self, pipeline: u64, from: u64, to: u64, fields: &[(u64, u64)]) {
        let mappings: Vec<Value> = fields
            .iter()
            .map(|(from, to)| json!([[from], [to]]))
            .collect();

        self.call(
            "connect",
            json!({
                "pipeline": pipeline,
                "from": from,
                "to": to,
                "mappings": mappings,
            }),
        );
    }

    pub fn connect_nodes(
        &mut self,
        pipeline: u64,
        from: u64,
        to: u64,
        from_field: u64,
        to_field: u64,
    ) {
        self.call(
            "connect",
            json!({
                "pipeline": pipeline,
                "from": from,
                "to": to,
                "mappings": [[[from_field], [to_field]]],
            }),
        );
    }

    /// file.changed on the source starts a new file on the dumper.
    pub fn subscribe_codec_change(&mut self, source: &str, dumper: &str) {
        self.call(
            "subscribe",
            json!({
                "from": source,
                "event": 0,
                "to": dumper,
                "command": 0,
                "mappings": [[[1], [0]], [[2], [1]]],
            }),
        );
    }

    pub fn run(&mut self, pipeline: u64, seconds: f32) {
        self.call("start", json!({ "pipeline": pipeline }));
        std::thread::sleep(Duration::from_secs_f32(seconds));
        self.call("stop", json!({ "pipeline": pipeline }));
        std::thread::sleep(Duration::from_millis(600));
    }

    /// Closes the dumper's current file.
    pub fn finish(&mut self, dumper: &str) {
        self.call(
            "invoke",
            json!({ "plugin": dumper, "command": 1, "payload": [] }),
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}

// ---------- media to test with ----------

fn has(tool: &str) -> bool {
    Command::new(tool)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// A media file to feed a pipeline, from the source plugin's own samples.
///
/// They are committed beside the plugin that reads them, so nothing has to be
/// encoded to run these; `plugins-src/mstu-media-file-source/test_samples`
/// holds them and the script that made them.
pub fn clip(name: &str) -> Option<PathBuf> {
    let path = center()
        .join("plugins-src/mstu-media-file-source/test_samples")
        .join(name);

    path.is_file().then_some(path)
}

/// What a file holds, as ffprobe reads it: codec and frames counted.
pub fn probe(path: &Path) -> Option<(String, u64)> {
    if !has("ffprobe") {
        return None;
    }

    let output = Command::new("ffprobe")
        .args(["-v", "error", "-count_frames", "-select_streams", "v:0"])
        .args(["-show_entries", "stream=codec_name,nb_read_frames"])
        .args(["-of", "csv=p=0"])
        .arg(path)
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.trim().split(',');

    let codec = parts.next()?.to_string();
    let frames = parts.next()?.parse().ok()?;

    (!codec.is_empty()).then_some((codec, frames))
}

/// The files written into a directory, in a settled order.
pub fn written(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = dir
        .read_dir()
        .expect("the output directory")
        .flatten()
        .map(|entry| entry.path())
        .collect();

    paths.sort();
    paths
}

/// Says why a test did nothing, rather than passing in silence.
pub fn skipped(why: &str) {
    println!("skipped: {why}");
}
