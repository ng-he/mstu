//! A camera for the RTSP source to dial.
//!
//! It speaks only what the plugin asks of it, and the video it sends is built
//! here byte by byte, so these tests need no camera and no network beyond the
//! loopback. Nothing is shared with the plugin's own fixtures: the centre
//! tests a plugin from the outside, the way the engine sees it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use std::{format, fs};

/// A real key frame, borrowed from the decoder plugin's own samples: a
/// camera that sends made up bytes is enough for a dumper, which writes
/// whatever it is given, and no use at all to a decoder.
fn keyframe() -> Vec<u8> {
    let path = crate::center().join("plugins-src/mstu-image-decoder/test_samples/h264.bin");

    fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Its units, split on their start codes.
///
/// Both lengths of start code: an encoder writes four bytes in front of a
/// parameter set and three in front of a picture, and splitting on the long
/// one alone leaves two units stuck together.
fn units() -> Vec<Vec<u8>> {
    let whole = keyframe();
    let mut units = Vec::new();
    let mut start: Option<usize> = None;
    let mut at = 0;

    while at + 3 <= whole.len() {
        if whole[at..at + 3] != [0, 0, 1] {
            at += 1;
            continue;
        }

        if let Some(from) = start {
            let mut end = at;

            // The four byte code is the three byte one with a zero in front.
            if end > from && whole[end - 1] == 0 {
                end -= 1;
            }

            units.push(whole[from..end].to_vec());
        }

        at += 3;
        start = Some(at);
    }

    if let Some(from) = start {
        units.push(whole[from..].to_vec());
    }

    units
}

/// The sets a decoder needs, and the picture that follows them.
fn sets_and_picture() -> (Vec<Vec<u8>>, Vec<u8>) {
    let mut units = units();
    let picture = units.pop().expect("a picture in the sample");

    (units, picture)
}

/// Ninety kilohertz, and a frame every twenty fifth of a second.
const TICKS: u32 = 90_000 / 25;

/// How often a frame goes out, which is slower than the clock says only so a
/// short run does not have to carry thousands of them.
const EVERY: Duration = Duration::from_millis(20);

/// The payload type the description offers.
const PAYLOAD_TYPE: u8 = 96;

pub struct Camera {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    sent: Arc<AtomicU64>,
    serving: Option<thread::JoinHandle<()>>,
}

impl Camera {
    /// Starts answering on a port of its own, and streams for as long as a
    /// client stays connected.
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port of our own");
        let address = listener.local_addr().expect("the port it took");

        let stop = Arc::new(AtomicBool::new(false));
        let sent = Arc::new(AtomicU64::new(0));

        let serving = {
            let stop = Arc::clone(&stop);
            let sent = Arc::clone(&sent);

            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let Ok((stream, _)) = listener.accept() else {
                        return;
                    };

                    serve(stream, &stop, &sent);
                }
            })
        };

        Self {
            address,
            stop,
            sent,
            serving: Some(serving),
        }
    }

    pub fn url(&self) -> String {
        format!("rtsp://{}/stream", self.address)
    }

    /// How many frames have gone out, so a test can wait for some to arrive
    /// rather than for a length of time.
    pub fn sent(&self) -> u64 {
        self.sent.load(Ordering::Relaxed)
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);

        if let Some(serving) = self.serving.take() {
            // Unblocks the accept that is waiting for the next client.
            let _ = TcpStream::connect(self.address);
            let _ = serving.join();
        }
    }
}

fn serve(stream: TcpStream, stop: &Arc<AtomicBool>, sent: &Arc<AtomicU64>) {
    let mut socket = stream.try_clone().expect("a handle to answer on");
    let mut lines = BufReader::new(stream);

    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }

        let Some(request) = read_request(&mut lines) else {
            return;
        };

        let line = request.first().cloned().unwrap_or_default();
        let method = line.split(' ').next().unwrap_or("").to_string();
        let sequence = header(&request, "CSeq").unwrap_or_else(|| "0".to_string());

        let answered = match method.as_str() {
            "DESCRIBE" => {
                let body = description();

                write!(
                    socket,
                    "RTSP/1.0 200 OK\r\n\
                     CSeq: {sequence}\r\n\
                     Content-Type: application/sdp\r\n\
                     Content-Length: {}\r\n\r\n{body}",
                    body.len()
                )
            }

            "SETUP" => write!(
                socket,
                "RTSP/1.0 200 OK\r\n\
                 CSeq: {sequence}\r\n\
                 Session: 99887766;timeout=60\r\n\
                 Transport: RTP/AVP/TCP;unicast;interleaved=0-1\r\n\r\n"
            ),

            "PLAY" => {
                let played = write!(
                    socket,
                    "RTSP/1.0 200 OK\r\nCSeq: {sequence}\r\nSession: 99887766\r\n\r\n"
                );

                if played.is_ok() {
                    // Streams until the client goes away or the test is over.
                    stream_video(&mut socket, stop, sent);
                    return;
                }

                played
            }

            "TEARDOWN" => {
                let _ = write!(socket, "RTSP/1.0 200 OK\r\nCSeq: {sequence}\r\n\r\n");
                return;
            }

            _ => write!(
                socket,
                "RTSP/1.0 200 OK\r\n\
                 CSeq: {sequence}\r\n\
                 Public: DESCRIBE, SETUP, PLAY, TEARDOWN\r\n\r\n"
            ),
        };

        if answered.is_err() {
            return;
        }
    }
}

/// The same key frame over and over: one picture that stands on its own is
/// all a decoder needs to prove it decodes.
fn stream_video(socket: &mut TcpStream, stop: &Arc<AtomicBool>, sent: &Arc<AtomicU64>) {
    let mut sequence = 0u16;
    let mut timestamp = 36_000u32;

    let (sets, picture) = sets_and_picture();

    for _ in 0.. {
        if stop.load(Ordering::Relaxed) {
            return;
        }

        let mut units: Vec<Vec<u8>> = sets.clone();

        units.push(picture.clone());

        for (at, unit) in units.iter().enumerate() {
            sequence = sequence.wrapping_add(1);

            let last = at + 1 == units.len();
            let packet = rtp(sequence, timestamp, last, unit);

            if socket.write_all(&interleaved(0, &packet)).is_err() {
                return;
            }
        }

        sent.fetch_add(1, Ordering::Relaxed);

        timestamp = timestamp.wrapping_add(TICKS);

        thread::sleep(EVERY);
    }
}

fn rtp(sequence: u16, timestamp: u32, marker: bool, payload: &[u8]) -> Vec<u8> {
    let mut packet = vec![0x80, PAYLOAD_TYPE | if marker { 0x80 } else { 0 }];

    packet.extend_from_slice(&sequence.to_be_bytes());
    packet.extend_from_slice(&timestamp.to_be_bytes());
    packet.extend_from_slice(&0x5151_5151u32.to_be_bytes());
    packet.extend_from_slice(payload);

    packet
}

fn interleaved(channel: u8, packet: &[u8]) -> Vec<u8> {
    let mut frame = vec![b'$', channel];

    frame.extend_from_slice(&(packet.len() as u16).to_be_bytes());
    frame.extend_from_slice(packet);

    frame
}

fn description() -> String {
    let (sets, _) = sets_and_picture();
    let sets = sets
        .iter()
        .map(|set| base64(set))
        .collect::<Vec<String>>()
        .join(",");

    format!(
        "v=0\r\n\
         o=- 0 0 IN IP4 127.0.0.1\r\n\
         s=Integration\r\n\
         t=0 0\r\n\
         a=control:*\r\n\
         m=video 0 RTP/AVP {PAYLOAD_TYPE}\r\n\
         a=rtpmap:{PAYLOAD_TYPE} H264/90000\r\n\
         a=fmtp:{PAYLOAD_TYPE} packetization-mode=1;sprop-parameter-sets={sets}\r\n\
         a=control:trackID=1\r\n"
    )
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut text = String::new();

    for chunk in bytes.chunks(3) {
        let mut word = 0u32;

        for (at, byte) in chunk.iter().enumerate() {
            word |= (*byte as u32) << (16 - 8 * at);
        }

        for at in 0..4 {
            if at <= chunk.len() {
                text.push(ALPHABET[(word >> (18 - 6 * at)) as usize & 0x3f] as char);
            } else {
                text.push('=');
            }
        }
    }

    text
}

fn read_request(lines: &mut BufReader<TcpStream>) -> Option<Vec<String>> {
    let mut request = Vec::new();

    loop {
        let mut line = String::new();

        if lines.read_line(&mut line).ok()? == 0 {
            return None;
        }

        let line = line.trim_end().to_string();

        if line.is_empty() {
            break;
        }

        request.push(line);
    }

    if let Some(length) = header(&request, "Content-Length").and_then(|value| value.parse().ok()) {
        let mut body = vec![0u8; length];

        lines.read_exact(&mut body).ok()?;
    }

    (!request.is_empty()).then_some(request)
}

fn header(request: &[String], name: &str) -> Option<String> {
    request.iter().find_map(|line| {
        let (key, value) = line.split_once(':')?;

        key.eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}
