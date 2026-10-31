//! A source and a dumper, wired through a running engine.

use std::time::Duration;

use mstu_integration::*;
use serde_json::json;

#[test]
fn the_engine_serves_the_plugins_that_were_collected() {
    let engine = Engine::start("libraries");
    let mut client = engine.connect();

    let described = client.call("describe", json!({}));
    let libraries = described["libraries"]
        .as_array()
        .expect("a library per plugin collected");

    let keys: Vec<&str> = libraries
        .iter()
        .filter_map(|library| library["key"].as_str())
        .collect();

    assert!(keys.contains(&SOURCE), "{keys:?}");
    assert!(keys.contains(&DUMPER), "{keys:?}");
}

/// The status bar's numbers: the engine says what it is costing, whether or
/// not anything is running.
#[test]
fn the_engine_says_what_it_is_costing() {
    let engine = Engine::start("load");
    let mut client = engine.connect();

    let pushed = client.pushes(Duration::from_millis(1500));

    let load = pushed
        .iter()
        .find(|message| message["type"] == "load")
        .unwrap_or_else(|| panic!("a load line: {pushed:?}"));

    assert!(
        load["memory"].as_u64().unwrap_or(0) > 1024 * 1024,
        "a running engine holds more than a megabyte: {load}"
    );

    assert!(load["cpu"].as_f64().unwrap_or(-1.0) >= 0.0, "{load}");

    // Nothing is wired, so the arenas hold nothing.
    assert_eq!(load["pipeline"], 0);
    assert_eq!(load["chunks"], 0);
}

#[test]
fn a_file_read_by_the_source_is_written_by_the_dumper() {
    let Some(clip) = clip("h264.mp4") else {
        return skipped("the source plugin's test samples are not checked out");
    };

    let engine = Engine::start("stream");
    let mut client = engine.connect();
    let out = scratch("stream-out");

    let pipeline = client.pipeline("stream");
    let source = client.plugin(SOURCE);
    let from = client.node(pipeline, &source);

    let dumper = client.plugin(DUMPER);
    let to = client.node(pipeline, &dumper);

    client.set(&dumper, 0, json!(out.to_string_lossy()));
    client.connect_nodes(pipeline, from, to, PAYLOAD, DATA);
    client.subscribe_codec_change(&source, &dumper);

    client.set(&source, 0, json!(clip.to_string_lossy()));
    client.run(pipeline, 3.0);
    client.finish(&dumper);

    let files = written(&out);
    assert_eq!(files.len(), 1, "{files:?}");

    let dump = &files[0];
    assert_eq!(dump.extension().unwrap(), "h264");

    let Some((codec, frames)) = probe(dump) else {
        return skipped("ffprobe is not installed, so the dump was not decoded");
    };

    assert_eq!(codec, "h264");
    assert_eq!(frames, 60, "the whole clip should come through");
}

/// The payload is shared rather than copied, so two dumpers of the same source
/// must end up with the same bytes.
#[test]
fn one_source_feeds_two_dumpers_the_same_bytes() {
    let Some(clip) = clip("h264.mp4") else {
        return skipped("the source plugin's test samples are not checked out");
    };

    let engine = Engine::start("fanout");
    let mut client = engine.connect();
    let out = scratch("fanout-out");

    let pipeline = client.pipeline("fanout");
    let source = client.plugin(SOURCE);
    let from = client.node(pipeline, &source);

    let mut dumpers = Vec::new();

    for _ in 0..2 {
        let dumper = client.plugin(DUMPER);
        let to = client.node(pipeline, &dumper);

        client.set(&dumper, 0, json!(out.to_string_lossy()));
        client.connect_nodes(pipeline, from, to, PAYLOAD, DATA);
        client.subscribe_codec_change(&source, &dumper);

        dumpers.push(dumper);
    }

    client.set(&source, 0, json!(clip.to_string_lossy()));
    client.run(pipeline, 3.0);

    for dumper in &dumpers {
        client.finish(dumper);
    }

    let files = written(&out);
    assert_eq!(files.len(), 2, "{files:?}");

    let first = std::fs::read(&files[0]).expect("the first dump");
    let second = std::fs::read(&files[1]).expect("the second dump");

    assert!(!first.is_empty());
    assert_eq!(first, second, "the two dumps should be identical");
}

/// Every container and codec the source claims to read, read.
#[test]
fn each_container_and_codec_opens_and_names_itself() {
    let cases: [(&str, &str); 6] = [
        ("h264.mp4", "h264"),
        ("h264.mov", "h264"),
        ("hvc1.mp4", "h265"),
        ("hev1.mp4", "h265"),
        ("av1.mp4", "av1"),
        ("vp9.mp4", "vp9"),
    ];

    let engine = Engine::start("codecs");
    let mut client = engine.connect();

    for (name, expected) in cases {
        let Some(clip) = clip(name) else {
            skipped(&format!("{name} is not checked out"));
            continue;
        };

        let source = client.plugin(SOURCE);

        client.set(&source, 0, json!(clip.to_string_lossy()));

        let published = client.pushes(Duration::from_millis(400));

        let codec = published
            .iter()
            .find(|message| message["type"] == "event" && message["plugin"] == source.as_str())
            .and_then(|message| message["values"][1].as_str())
            .unwrap_or_else(|| panic!("{name}: no file.changed carrying a codec"));

        assert_eq!(codec, expected, "{name} should be read as {expected}");

        client.call("remove_plugin", json!({ "plugin": source }));
    }
}

/// A file the reader cannot make sense of is refused, and says why.
#[test]
fn something_that_is_not_media_is_turned_down() {
    let engine = Engine::start("refuse");
    let mut client = engine.connect();
    let dir = scratch("refuse-in");

    let not_media = dir.join("script.mp4");
    std::fs::write(&not_media, b"@echo off\r\nexit\r\n").expect("a file that is not media");

    let source = client.plugin(SOURCE);
    let refused = client.try_call(
        "set_parameter",
        json!({ "plugin": source, "field": 0, "value": not_media.to_string_lossy() }),
    );

    assert!(refused.is_err(), "a batch file is not a video");
}

/// The dumper is driven by the source's events, not only by its samples.
#[test]
fn the_codec_event_names_the_file_the_dumper_opens() {
    let Some(clip) = clip("av1.mp4") else {
        return skipped("the source plugin's test samples are not checked out");
    };

    let engine = Engine::start("event");
    let mut client = engine.connect();
    let out = scratch("event-out");

    let pipeline = client.pipeline("event");
    let source = client.plugin(SOURCE);
    let from = client.node(pipeline, &source);

    let dumper = client.plugin(DUMPER);
    let to = client.node(pipeline, &dumper);

    client.set(&dumper, 0, json!(out.to_string_lossy()));
    client.connect_nodes(pipeline, from, to, PAYLOAD, DATA);
    client.subscribe_codec_change(&source, &dumper);

    // Opening the file is enough: the event alone opens the dump.
    client.set(&source, 0, json!(clip.to_string_lossy()));
    std::thread::sleep(Duration::from_millis(400));
    client.finish(&dumper);

    let files = written(&out);

    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(
        files[0].extension().unwrap(),
        "obu",
        "an av1 stream is named for what it is"
    );
}

/// What the plugins report about themselves, which the desktop draws.
#[test]
fn plugins_report_live_values_while_they_run() {
    let Some(clip) = clip("h264.mp4") else {
        return skipped("the source plugin's test samples are not checked out");
    };

    let engine = Engine::start("live");
    let mut client = engine.connect();
    let out = scratch("live-out");

    let pipeline = client.pipeline("live");
    let source = client.plugin(SOURCE);
    let from = client.node(pipeline, &source);

    let dumper = client.plugin(DUMPER);
    let to = client.node(pipeline, &dumper);

    client.set(&dumper, 0, json!(out.to_string_lossy()));
    client.connect_nodes(pipeline, from, to, PAYLOAD, DATA);
    client.subscribe_codec_change(&source, &dumper);
    client.set(&source, 0, json!(clip.to_string_lossy()));

    client.call("start", json!({ "pipeline": pipeline }));
    let published = client.pushes(Duration::from_millis(1500));
    client.call("stop", json!({ "pipeline": pipeline }));

    let live: Vec<&serde_json::Value> = published
        .iter()
        .filter(|message| message["type"] == "live")
        .collect();

    assert!(!live.is_empty(), "nothing was published while running");

    // The source counts samples; by now it has read some.
    let samples = live
        .iter()
        .filter(|message| message["plugin"] == source.as_str())
        .filter_map(|message| message["values"][0].as_u64())
        .max()
        .expect("the source to report its samples");

    assert!(samples > 0, "the source read nothing");
}
