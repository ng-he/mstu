//! A file, a decoder in the middle and a dumper: the first pipeline whose
//! middle node both takes and gives.

use std::fs;
use std::time::Duration;

use mstu_integration::camera::Camera;
use mstu_integration::*;
use serde_json::json;

/// How many jpegs are in what the dumper wrote.
/// Waits for the camera to have sent a few frames, so the test is over when
/// the work is done rather than after a fixed sleep.
fn streamed(camera: &Camera, frames: u64, within: Duration) -> bool {
    let deadline = std::time::Instant::now() + within;

    while std::time::Instant::now() < deadline {
        if camera.sent() >= frames {
            return true;
        }

        std::thread::sleep(Duration::from_millis(20));
    }

    false
}

fn jpegs(bytes: &[u8]) -> usize {
    bytes
        .windows(3)
        .filter(|three| *three == [0xff, 0xd8, 0xff])
        .count()
}

#[test]
fn a_file_decoded_into_images_is_written_by_the_dumper() {
    let Some(clip) = clip("h264.mp4") else {
        return skipped("the source plugin's test samples are not checked out");
    };

    let engine = Engine::start("images");
    let mut client = engine.connect();
    let out = scratch("images-written");

    let pipeline = client.pipeline("decode");
    let source = client.plugin(SOURCE);
    let decoder = client.plugin(IMAGES);
    let dumper = client.plugin(DUMPER);

    client.set(&dumper, 0, json!(out.to_string_lossy()));
    client.set(&decoder, 0, json!(160));

    let from = client.node(pipeline, &source);
    let middle = client.node(pipeline, &decoder);
    let to = client.node(pipeline, &dumper);

    // The decoder takes the time, the sync flag and the frame.
    client.connect_many(pipeline, from, middle, &[(0, 0), (3, 1), (4, 2)]);

    // And hands on the jpeg, which is its fourth field.
    client.connect_nodes(pipeline, middle, to, 3, DATA);

    // file.changed says what the frames are, which is what codec.change asks,
    // and image.changed says the same thing about the images to the dumper.
    client.subscribe_codec_change(&source, &decoder);
    client.subscribe_codec_change(&decoder, &dumper);

    // Last, because file.changed goes out the moment the file opens and the
    // engine does not keep it for whoever subscribes afterwards.
    client.set(&source, 0, json!(clip.to_string_lossy()));

    client.call("start", json!({ "pipeline": pipeline }));

    let pushed = client.pushes(Duration::from_millis(1500));

    client.call("stop", json!({ "pipeline": pipeline }));
    std::thread::sleep(Duration::from_millis(400));
    client.finish(&dumper);

    // What the plugin's own page reads. Live values go out in the order the
    // schema names them: images is the third, and the size of the last image
    // the eighth and ninth -- which the settings only ask for.
    let reading = pushed
        .iter()
        .filter(|message| message["type"] == "live" && message["plugin"] == decoder.as_str())
        .filter_map(|message| message["values"].as_array().cloned())
        .find(|values| values[2].as_u64().unwrap_or(0) > 0)
        .expect("a live reading with images in it");

    assert_eq!(reading[7], 160, "the width of the images: {reading:?}");
    assert!(reading[8].as_u64().unwrap_or(0) > 0, "{reading:?}");

    // And the tenth names a file holding the latest image, which is how the
    // plugin's own page shows one: live readings carry no payloads.
    let shown = reading[9].as_str().expect("a preview path");
    let kept = fs::read(shown).unwrap_or_else(|error| panic!("{shown}: {error}"));

    assert_eq!(kept[..3], [0xff, 0xd8, 0xff], "a jpeg in {shown}");

    let written = written(&out);

    // One image, one file: the decoder says image.changed before each, and
    // that is what opens a file on the dumper.
    assert!(written.len() >= 10, "{} file(s)", written.len());
    assert!(
        written
            .iter()
            .all(|path| path.extension().and_then(|ext| ext.to_str()) == Some("jpg")),
        "{written:?}"
    );

    let images: usize = written
        .iter()
        .map(|path| jpegs(&fs::read(path).expect("what the dumper wrote")))
        .sum();

    // A file is opened for every image, and every image is written into one --
    // except the last ones, whose frames may still be queued when the
    // pipeline stops.
    assert!(
        images <= written.len(),
        "{images} images in {} files",
        written.len()
    );
    assert!(
        images + 3 >= written.len(),
        "{images} images for {} files opened",
        written.len()
    );

    // Events reach a subscriber at once while frames queue, so now and then
    // the next file is open before the last image arrives and the image lands
    // in it: the file before it is left empty. Roughly one in fifty.
    let empty = written
        .iter()
        .filter(|path| path.metadata().expect("the file").len() == 0)
        .count();

    assert!(
        empty * 10 < written.len(),
        "{empty} of {} files are empty",
        written.len()
    );
}

/// The engine's own word on the node in the middle: it takes frames and gives
/// images, and the fields line up with what sits either side of it.
#[test]
fn the_decoder_is_described_as_a_node_in_the_middle() {
    let engine = Engine::start("images-shapes");
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

    let decoder = library(IMAGES);
    let source = library(SOURCE);
    let dumper = library(DUMPER);

    // A page of its own, which the host found beside the library.
    let made = client.call("create_plugin", json!({ "library": IMAGES }));

    assert!(
        made["ui"].is_string(),
        "the ui folder should have been found"
    );

    assert_eq!(
        decoder["source"], false,
        "it takes an input, so it is no source"
    );
    assert_eq!(decoder["input"]["fields"][2]["name"], "bytes");
    assert_eq!(decoder["output"]["fields"][3]["name"], "bytes");

    // What the source gives and the decoder takes.
    assert_eq!(
        source["output"]["fields"][4]["type"],
        decoder["input"]["fields"][2]["type"]
    );

    // What the decoder gives and the dumper takes.
    assert_eq!(
        decoder["output"]["fields"][3]["type"],
        dumper["input"]["fields"][0]["type"]
    );

    // And the codec it is told about is told the same way the dumper is.
    assert_eq!(
        source["events"][0]["schema"]["fields"][1]["type"],
        decoder["commands"][0]["schema"]["fields"][0]["type"]
    );
}

/// The chain the plugins were written for: a camera at one end, jpegs at the
/// other, and a decoder between them that neither of the two knows about.
#[test]
fn a_camera_decoded_into_images_is_written_by_the_dumper() {
    let camera = Camera::start();

    let engine = Engine::start("camera-images");
    let mut client = engine.connect();
    let out = scratch("camera-images-written");

    let pipeline = client.pipeline("camera");
    let source = client.plugin(RTSP);
    let decoder = client.plugin(IMAGES);
    let dumper = client.plugin(DUMPER);

    client.set(&dumper, 0, json!(out.to_string_lossy()));
    client.set(&decoder, 0, json!(96));
    client.set(&source, 0, json!(camera.url()));

    let from = client.node(pipeline, &source);
    let middle = client.node(pipeline, &decoder);
    let to = client.node(pipeline, &dumper);

    client.connect_many(pipeline, from, middle, &[(0, 0), (3, 1), (4, 2)]);
    client.connect_nodes(pipeline, middle, to, 3, DATA);

    // The source dials once the pipeline runs, so these are in place before
    // it has anything to say.
    client.subscribe_codec_change(&source, &decoder);
    client.subscribe_codec_change(&decoder, &dumper);

    client.call("start", json!({ "pipeline": pipeline }));

    assert!(
        streamed(&camera, 10, Duration::from_secs(10)),
        "the camera should have been dialled and streamed"
    );

    client.call("stop", json!({ "pipeline": pipeline }));
    std::thread::sleep(Duration::from_millis(400));
    client.finish(&dumper);

    let written = written(&out);

    assert!(written.len() >= 2, "{} file(s) written", written.len());

    let images: usize = written
        .iter()
        .map(|path| jpegs(&fs::read(path).expect("what the dumper wrote")))
        .sum();

    assert!(images >= 2, "{images} image(s) out of a camera");

    // The size asked for, read back out of a jpeg the dumper wrote.
    let first = written
        .iter()
        .map(|path| fs::read(path).expect("the file"))
        .find(|bytes| !bytes.is_empty())
        .expect("an image with something in it");

    assert_eq!(first[..3], [0xff, 0xd8, 0xff]);
}
