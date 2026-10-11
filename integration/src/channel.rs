//! The engine's local channel, from the side the desktop app sits on.
//!
//! A unix socket here, a named pipe on Windows. The protocol above it is the
//! same, so the rest of the harness sees one thing: lines in, lines out, with
//! a deadline on waiting for them.

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

/// A channel of this run's own, named after the directory it was given.
///
/// Two engines must not answer to one name, which matters here more than
/// anywhere: the tests run side by side.
#[cfg(unix)]
pub fn address(runtime: &Path) -> String {
    runtime.join("mstu.sock").to_string_lossy().into_owned()
}

#[cfg(windows)]
pub fn address(runtime: &Path) -> String {
    let name = runtime
        .file_name()
        .expect("the run's own directory")
        .to_string_lossy();

    format!(r"\\.\pipe\{name}")
}

/// Waits for an engine to answer on `address`, as a client would.
///
/// A pipe is no file, so there is nothing on disk to watch for; knocking is
/// the one way that works on both.
pub fn wait_until_served(address: &str, within: Duration) -> io::Result<()> {
    let since = Instant::now();

    loop {
        match Channel::try_connect(address) {
            Ok(_) => return Ok(()),
            Err(error) if since.elapsed() >= within => return Err(error),
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

#[cfg(unix)]
mod platform {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    pub struct Channel {
        stream: UnixStream,
        lines: BufReader<UnixStream>,
    }

    impl Channel {
        pub fn try_connect(address: &str) -> std::io::Result<Self> {
            let stream = UnixStream::connect(address)?;
            let lines = BufReader::new(stream.try_clone()?);

            Ok(Self { stream, lines })
        }

        pub fn send(&mut self, line: &str) -> Result<(), String> {
            writeln!(self.stream, "{line}").map_err(|error| error.to_string())
        }

        pub fn line(&mut self, within: Duration) -> Result<String, String> {
            self.stream
                .set_read_timeout(Some(within))
                .map_err(|error| error.to_string())?;

            let mut line = String::new();

            match self.lines.read_line(&mut line) {
                Ok(0) => Err("the engine closed the connection".to_string()),
                Ok(_) => Ok(line),
                Err(error) => Err(format!("nothing came back: {error}")),
            }
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::fs::{File, OpenOptions};
    use std::io::{BufRead, BufReader, Write};
    use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
    use std::time::Duration;

    pub struct Channel {
        pipe: File,
        lines: Receiver<std::io::Result<String>>,
    }

    impl Channel {
        pub fn try_connect(address: &str) -> std::io::Result<Self> {
            let pipe = OpenOptions::new().read(true).write(true).open(address)?;

            // A pipe handle takes no read timeout, so a thread does the
            // waiting and the channel carries the deadline.
            let reading = pipe.try_clone()?;
            let (sender, lines) = channel();

            std::thread::spawn(move || {
                let mut reader = BufReader::new(reading);

                loop {
                    let mut line = String::new();

                    let read = match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => Ok(line),
                        Err(error) => Err(error),
                    };

                    let failed = read.is_err();

                    if sender.send(read).is_err() || failed {
                        break;
                    }
                }
            });

            Ok(Self { pipe, lines })
        }

        pub fn send(&mut self, line: &str) -> Result<(), String> {
            writeln!(&self.pipe, "{line}").map_err(|error| error.to_string())
        }

        pub fn line(&mut self, within: Duration) -> Result<String, String> {
            match self.lines.recv_timeout(within) {
                Ok(Ok(line)) => Ok(line),
                Ok(Err(error)) => Err(format!("nothing came back: {error}")),
                Err(RecvTimeoutError::Timeout) => Err("nothing came back in time".to_string()),
                Err(RecvTimeoutError::Disconnected) => {
                    Err("the engine closed the connection".to_string())
                }
            }
        }
    }
}

pub use platform::Channel;
