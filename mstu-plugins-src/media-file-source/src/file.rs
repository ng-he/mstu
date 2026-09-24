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

/// A sample without its bytes, known before they are read.
#[derive(Debug, Clone, Copy)]
pub struct SampleInfo {
    pub start_time: u64,
    pub duration: u32,
    pub rendering_offset: i32,
    pub is_sync: bool,
    pub size: usize,
}

impl fmt::Display for SampleInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "start_time {}, duration {}, rendering_offset {}, is_sync {}, length {}",
            self.start_time, self.duration, self.rendering_offset, self.is_sync, self.size
        )
    }
}

pub trait SampleReader {
    /// Describes the next sample without reading it, so its bytes can be read
    /// into the memory they will travel in. Does not advance.
    fn peek_sample(&mut self) -> Result<Option<SampleInfo>, String>;

    /// Reads the sample `peek_sample` described into `into`, which is exactly
    /// `size` bytes, and moves on to the next one.
    fn read_sample(&mut self, into: &mut [u8]) -> Result<(), String>;

    fn codec(&self) -> mstu_media::Codec;

    /// Units per second that sample times are counted in.
    ///
    /// A raw elementary stream carries no clock, so it reports whatever rate
    /// it was told to assume.
    fn timescale(&self) -> u32;

    /// Total length in timescale units, `None` when the source cannot know it
    /// without reading to the end.
    fn duration(&self) -> Option<u64> {
        None
    }

    /// Back to the first sample. Every source can do this.
    fn rewind(&mut self) -> Result<(), String>;

    /// Moves to `time` in timescale units, snapped back to a keyframe.
    ///
    /// False when the source has no index to seek with.
    fn seek(&mut self, _time: u64) -> bool {
        false
    }
}
