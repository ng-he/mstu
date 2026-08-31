use std::{fmt, write};

use bytes::Bytes;

#[derive(Debug, Clone)]
pub struct Sample {
    pub start_time: u64,
    pub duration: u32,
    pub rendering_offset: i32,
    pub is_sync: bool,
    pub bytes: Bytes,
}

impl fmt::Display for Sample {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "start_time {}, duration {}, rendering_offset {}, is_sync {}, length {}",
            self.start_time,
            self.duration,
            self.rendering_offset,
            self.is_sync,
            self.bytes.len()
        )
    }
}

pub trait SampleReader {
    fn next_sample(&mut self) -> Result<Option<Sample>, String>;
    fn codec(&self) -> mstu_media::Codec;
}
