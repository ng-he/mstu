//! A camera, the RTSP source and a dumper, wired through a running engine.

use std::fs;
use std::time::{Duration, Instant};

use mstu_integration::camera::Camera;
use mstu_integration::*;
use serde_json::json;

/// Waits for the camera to have sent a few frames, so the test is over when
/// the work is done rather than after a fixed sleep.
fn streamed(camera: &Camera, frames: u64, within: Duration) -> bool {
    let deadline = Instant::now() + within;

    while Instant::now() < deadline {
        if camera.sent() >= frames {
            return true;
        }

        std::thread::sleep(Duration::from_millis(20));
    }

    false
}

#[test]
fn a_camera_read_by_the_rtsp_source_is_written_by_the_dumper() {
    let camera = Camera::start();

    let engine = Engine::start("rtsp");
    let mut client = engine.connect();

    let out = scratch("rtsp-written");

    let pipeline = client.pipeline("camera");
    let source = client.plugin(RTSP);
    let dumper = client.plugin(DUMPER);

    client.set(&dumper, 0, json!(out.to_string_lossy()));
    client.set(&source, 0, json!(camera.url()));

    let from = client.node(pipeline, &source);
    let to = client.node(pipeline, &dumper);

    client.connect_nodes(pipeline, from, to, PAYLOAD, DATA);

    // stream.changed opens a file on the dumper, as file.changed does.
    client.subscribe_codec_change(&source, &dumper);

    client.call("start", json!({ "pipeline": pipeline }));

    assert!(
        streamed(&camera, 10, Duration::from_secs(10)),
        "the camera should have been dialled and streamed"
    );

    client.call("stop", json!({ "pipeline": pipeline }));
    std::thread::sleep(Duration::from_millis(400));
    client.finish(&dumper);

    let written = written(&out);

    assert_eq!(
        written.len(),
        1,
        "one file, named for the codec: {written:?}"
    );
    assert_eq!(
        written[0].extension().and_then(|ext| ext.to_str()),
        Some("h264"),
        "the codec came through stream.changed"
    );

    let bytes = fs::read(&written[0]).expect("the file the dumper wrote");

    // Annex-B, with the parameter sets the camera offered in front.
    assert_eq!(bytes[..4], [0, 0, 0, 1]);
    assert_eq!(bytes[4] & 0x1f, 7, "an sps first, so a decoder can start");
    assert!(
        bytes.windows(4).filter(|w| *w == [0, 0, 0, 1]).count() >= 10,
        "a start code per unit, and there were at least ten"
    );
    assert!(bytes.len() > 2_000, "{} byte(s) is too few", bytes.len());
}

/// What the source says and what the dumper is told have to line up field for
/// field, or the wiring only looks right.
#[test]
fn the_source_tells_the_dumper_what_it_asks_for() {
    let engine = Engine::start("rtsp-shapes");
    let mut client = engine.connect();

    let described = client.call("describe", json!({}));
    let libraries = described["libraries"]
        .as_array()
        .cloned()
        .expect("a library per plugin");

    let library = |key: &str| {
        libraries
            .iter()
            .find(|library| library["key"] == key)
            .cloned()
            .unwrap_or_else(|| panic!("{key} should have been collected"))
    };

    let source = library(RTSP);
    let dumper = library(DUMPER);

    // A source takes nothing in and hands video out.
    assert_eq!(source["source"], true);
    assert_eq!(source["output"]["fields"][4]["name"], "bytes");
    assert_eq!(source["output"]["fields"][4]["type"], "bytes");

    let changed = &source["events"][0];
    let codec_change = &dumper["commands"][0];

    assert_eq!(changed["name"], "stream.changed");
    assert_eq!(codec_change["name"], "codec.change");

    // The event's second and third fields are the command's first and second,
    // which is the mapping anything downstream will be wired with.
    assert_eq!(
        changed["schema"]["fields"][1]["type"],
        codec_change["schema"]["fields"][0]["type"]
    );
    assert_eq!(
        changed["schema"]["fields"][2]["type"],
        codec_change["schema"]["fields"][1]["type"]
    );

    assert_eq!(source["events"][1]["name"], "stream.ended");
    assert_eq!(source["commands"][0]["name"], "stream.reconnect");
}

/// A camera that is not there is the normal state of affairs for a while
/// after a pipeline starts, and must not take the pipeline with it.
#[test]
fn a_camera_that_is_not_there_leaves_the_pipeline_running() {
    let engine = Engine::start("rtsp-nowhere");
    let mut client = engine.connect();

    let pipeline = client.pipeline("nowhere");
    let source = client.plugin(RTSP);

    // Nothing listens on port one.
    client.set(&source, 0, json!("rtsp://127.0.0.1:1/x"));
    client.node(pipeline, &source);

    client.call("start", json!({ "pipeline": pipeline }));
    std::thread::sleep(Duration::from_millis(800));

    let pushed = client.pushes(Duration::from_millis(600));

    // The engine is still answering, and the node is still a node.
    let described = client.call("describe", json!({}));

    assert!(described["libraries"].is_array());

    let live = pushed
        .iter()
        .filter(|message| message["type"] == "live")
        .count();

    assert!(live > 0, "the source should still be reporting: {pushed:?}");

    client.call("stop", json!({ "pipeline": pipeline }));
}
