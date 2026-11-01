//! The first node in a pipeline that is not written in Rust.
//!
//! Nothing detects anything yet: what these check is that a plugin built by
//! cmake from the C header alone is a node like any other -- described,
//! settable, wireable, and able to hand on what it was given.

use std::fs;
use std::time::Duration;

use mstu_integration::*;
use serde_json::json;

fn jpegs(bytes: &[u8]) -> usize {
    bytes
        .windows(3)
        .filter(|three| *three == [0xff, 0xd8, 0xff])
        .count()
}

#[test]
fn a_plugin_written_in_cpp_is_described_like_any_other() {
    let engine = Engine::start("vision-described");
    let mut client = engine.connect();

    let described = client.call("describe", json!({}));
    let libraries = described["libraries"]
        .as_array()
        .cloned()
        .expect("a library per plugin collected");

    let vision = libraries
        .iter()
        .find(|library| library["key"] == VISION)
        .cloned()
        .unwrap_or_else(|| panic!("{VISION} should have been collected from its dist folder"));

    assert_eq!(vision["name"], "Computer vision");
    assert_eq!(
        vision["source"], false,
        "it takes images, so it is no source"
    );

    // The four fields the image decoder gives, taken in the same order, which
    // is what lets this node sit anywhere images already flow.
    let decoder = libraries
        .iter()
        .find(|library| library["key"] == IMAGES)
        .cloned()
        .expect("the image decoder");

    for at in 0..4 {
        assert_eq!(
            decoder["output"]["fields"][at]["name"], vision["input"]["fields"][at]["name"],
            "field {at}"
        );
        assert_eq!(
            decoder["output"]["fields"][at]["type"], vision["input"]["fields"][at]["type"],
            "field {at}"
        );
    }

    // And it gives those four again with the detections behind them: a list of
    // records, which nothing had put through the engine before.
    // A field is described as its type with a name put on it, so the kind is
    // the field's own "type" and a list's element sits beside it.
    let objects = &vision["output"]["fields"][4];

    assert_eq!(objects["name"], "objects");
    assert_eq!(objects["type"], "list");
    assert_eq!(objects["element"]["type"], "record");

    let shape = objects["element"]["fields"]
        .as_array()
        .expect("the fields of one detection");

    assert_eq!(shape.len(), 7);
    assert_eq!(shape[0]["name"], "class");
    assert_eq!(shape[3]["name"], "x");
    assert_eq!(shape[3]["bits"], 32);

    // image.changed is its first event, as it is on a source and on the
    // decoder, so one subscription downstream reaches any of them.
    assert_eq!(vision["events"][0]["name"], "image.changed");
    assert_eq!(
        decoder["events"][0]["schema"]["fields"][1]["type"],
        vision["commands"][0]["schema"]["fields"][0]["type"],
        "what the decoder says and what this is asked"
    );
}

/// A file, a decoder, the C++ node, and a dumper: four nodes, and the one in
/// the middle built by cmake from the generated header.
#[test]
fn images_pass_through_a_cpp_node_on_their_way_to_the_dumper() {
    let Some(clip) = clip("h264.mp4") else {
        return skipped("the source plugin's test samples are not checked out");
    };

    let engine = Engine::start("vision-chain");
    let out = scratch("vision-written");
    let (pushed, written) = run_chain(&engine, &clip, &out, false);

    // What the C++ node's own readings say: images is the second field, and
    // it only moves if messages reached a plugin the engine loaded from a
    // library cmake built.
    let reading = pushed
        .iter()
        .find(|values| values[1].as_u64().unwrap_or(0) > 0)
        .expect("a live reading with images in it");

    assert_eq!(reading[0], "detecting", "{reading:?}");
    assert_eq!(reading[2], 0, "nothing detects yet: {reading:?}");

    assert!(!written.is_empty(), "the dumper wrote nothing");

    let images: usize = written
        .iter()
        .map(|path| jpegs(&fs::read(path).expect("what the dumper wrote")))
        .sum();

    // Every image the decoder made came out the far side of the C++ node.
    assert!(images > 0, "{images} images in {} files", written.len());
}

/// Nothing calls a plugin's `settings`, so the only honest way to see that a
/// setting arrived is to set one that changes what the plugin does.
///
/// Asked for only the images something was found in, and finding nothing yet,
/// it hands on nothing -- so the dumper gets no image at all.
#[test]
fn a_setting_written_from_the_app_changes_what_a_cpp_node_does() {
    let Some(clip) = clip("h264.mp4") else {
        return skipped("the source plugin's test samples are not checked out");
    };

    let engine = Engine::start("vision-setting");
    let out = scratch("vision-withheld");
    let (pushed, written) = run_chain(&engine, &clip, &out, true);

    // It saw the images: the setting changed what it did with them, not
    // whether they arrived.
    let seen = pushed
        .iter()
        .filter_map(|values| values[1].as_u64())
        .max()
        .unwrap_or(0);

    assert!(seen > 0, "the node took no image in: {pushed:?}");

    let images: usize = written
        .iter()
        .map(|path| jpegs(&fs::read(path).expect("what the dumper wrote")))
        .sum();

    // A file may have been opened, since image.changed still went out, but
    // no image was ever written into it.
    assert_eq!(images, 0, "{images} images in {} files", written.len());
}

/// Source, decoder, the C++ node and dumper, run once.
///
/// Returns the node's own live readings and what the dumper wrote.
fn run_chain(
    engine: &Engine,
    clip: &std::path::Path,
    out: &std::path::Path,
    only_when_found: bool,
) -> (Vec<Vec<serde_json::Value>>, Vec<std::path::PathBuf>) {
    let mut client = engine.connect();

    let pipeline = client.pipeline("look");
    let source = client.plugin(SOURCE);
    let decoder = client.plugin(IMAGES);
    let vision = client.plugin(VISION);
    let dumper = client.plugin(DUMPER);

    client.set(&dumper, 0, json!(out.to_string_lossy()));
    client.set(&decoder, 0, json!(160));

    if only_when_found {
        client.set(&vision, 4, json!(true));
    }

    let from = client.node(pipeline, &source);
    let decode = client.node(pipeline, &decoder);
    let look = client.node(pipeline, &vision);
    let to = client.node(pipeline, &dumper);

    // The decoder takes the time, the sync flag and the frame.
    client.connect_many(pipeline, from, decode, &[(0, 0), (3, 1), (4, 2)]);

    // The C++ node takes what the decoder gives, field for field.
    client.connect_many(pipeline, decode, look, &[(0, 0), (1, 1), (2, 2), (3, 3)]);

    // And hands the image on from the same place the decoder did.
    client.connect_nodes(pipeline, look, to, 3, DATA);

    client.subscribe_codec_change(&source, &decoder);
    client.subscribe_codec_change(&decoder, &vision);
    client.subscribe_codec_change(&vision, &dumper);

    // Last: an event goes out when it happens and the engine keeps none of
    // them for whoever subscribes afterwards.
    client.set(&source, 0, json!(clip.to_string_lossy()));

    client.call("start", json!({ "pipeline": pipeline }));

    let pushed = client.pushes(Duration::from_millis(1500));

    client.call("stop", json!({ "pipeline": pipeline }));
    std::thread::sleep(Duration::from_millis(400));
    client.finish(&dumper);

    let readings = pushed
        .iter()
        .filter(|message| message["type"] == "live" && message["plugin"] == vision.as_str())
        .filter_map(|message| message["values"].as_array().cloned())
        .collect();

    (readings, written(out))
}
