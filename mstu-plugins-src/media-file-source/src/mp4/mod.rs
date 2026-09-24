use bytes::Bytes;

use std::{
    collections::HashMap,
    fs::{self},
    io::{BufReader, Read, Seek},
    matches,
    path::PathBuf,
    time::Duration,
    write,
};

use crate::{
    file::{self, Sample, SampleInfo},
    media::FourCC,
};
use error::Error;

pub mod atom;
pub mod error;
pub mod track;

pub use track::Track;
pub use track::TrackType;

pub type Result<T> = std::result::Result<T, Error>;

pub struct Reader {
    ftyp: atom::Ftyp,
    moov: atom::Moov,
    tracks: HashMap<u32, Track>,
    size: u64,
}

impl Reader {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Reader> {
        let start = reader.stream_position()?;

        let mut ftyp = None;
        let mut moov = None;
        let mut current = start;

        while current < size {
            let header = atom::read_header(reader)?;
            if header.size > size {
                return Err(Error::InvalidData(
                    "Reader contains a atom with a larger size than it",
                ));
            }

            // Break if size zero Header, which can result in dead-loop.
            if header.size == 0 {
                break;
            }

            match header.name {
                atom::constant_name::FTYP => ftyp = Some(atom::Ftyp::read(reader, header.size)?),
                atom::constant_name::MOOV => moov = Some(atom::Moov::read(reader, header.size)?),
                _ => {
                    atom::read_skip(reader, header.size)?;
                }
            }
            current = reader.stream_position()?;
        }

        if ftyp.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::FTYP));
        }

        if moov.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::MOOV));
        }

        let size = current - start;
        let tracks = if let Some(ref moov) = moov {
            if moov.traks.iter().any(|trak| trak.tkhd.track_id == 0) {
                return Err(Error::InvalidData("illegal track id 0"));
            }

            moov.traks
                .iter()
                .map(|trak| (trak.tkhd.track_id, Track::from(trak)))
                .collect()
        } else {
            HashMap::new()
        };

        Ok(Reader {
            ftyp: ftyp.unwrap(),
            moov: moov.unwrap(),
            tracks: tracks,
            size: size,
        })
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn major_brand(&self) -> &FourCC {
        &self.ftyp.major_brand
    }

    pub fn minor_version(&self) -> u32 {
        self.ftyp.minor_version
    }

    pub fn compatible_brands(&self) -> &[FourCC] {
        &self.ftyp.compatible_brands
    }

    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.moov.mvhd.duration * 1000 / self.moov.mvhd.timescale as u64)
    }

    pub fn timescale(&self) -> u32 {
        self.moov.mvhd.timescale
    }

    pub fn tracks(&self) -> &HashMap<u32, Track> {
        &self.tracks
    }

    pub fn sample_count(&self, track_id: u32) -> Result<u32> {
        if let Some(track) = self.tracks.get(&track_id) {
            Ok(track.sample_count())
        } else {
            Err(Error::TrakNotFound(track_id))
        }
    }

    pub fn sample_offset(&mut self, track_id: u32, sample_id: u32) -> Result<u64> {
        if let Some(track) = self.tracks.get(&track_id) {
            track.sample_offset(sample_id)
        } else {
            Err(Error::TrakNotFound(track_id))
        }
    }

    pub fn read_sample<R: Read + Seek>(
        &mut self,
        reader: &mut R,
        track_id: u32,
        sample_id: u32,
    ) -> Result<Option<Sample>> {
        if let Some(track) = self.tracks.get(&track_id) {
            track.read_sample(reader, sample_id)
        } else {
            Err(Error::TrakNotFound(track_id))
        }
    }

    pub fn sample_info(&self, track_id: u32, sample_id: u32) -> Result<Option<SampleInfo>> {
        match self.tracks.get(&track_id) {
            Some(track) => track.sample_info(sample_id),
            None => Err(Error::TrakNotFound(track_id)),
        }
    }

    pub fn read_sample_into<R: Read + Seek>(
        &self,
        reader: &mut R,
        track_id: u32,
        sample_id: u32,
        into: &mut [u8],
    ) -> Result<()> {
        match self.tracks.get(&track_id) {
            Some(track) => track.read_sample_into(reader, sample_id, into),
            None => Err(Error::TrakNotFound(track_id)),
        }
    }

    /// Annex-B is the same size as AVCC only with 4-byte prefixes; anything
    /// shorter grows, so its sample cannot be read straight into place.
    pub fn reads_in_place(&self, track_id: u32) -> bool {
        self.tracks
            .get(&track_id)
            .is_some_and(|track| matches!(track.nalu_length_size(), None | Some(4)))
    }
}

pub struct File {
    buf_reader: BufReader<fs::File>,
    reader: Reader,
    video_track_id: u32,
    current_sample_id: u32,

    /// Only for prefixes shorter than 4 bytes, where Annex-B does not fit
    /// where AVCC was.
    converted: Option<Bytes>,
}

impl File {
    fn peek_converted(&mut self) -> std::result::Result<Option<SampleInfo>, String> {
        if self.converted.is_none() {
            let sample = self
                .reader
                .read_sample(
                    &mut self.buf_reader,
                    self.video_track_id,
                    self.current_sample_id,
                )
                .map_err(|err| err.to_string())?;

            let Some(sample) = sample else {
                return Ok(None);
            };

            self.converted = Some(sample.bytes.clone());

            return Ok(Some(SampleInfo {
                start_time: sample.start_time,
                duration: sample.duration,
                rendering_offset: sample.rendering_offset,
                is_sync: sample.is_sync,
                size: sample.bytes.len(),
            }));
        }

        self.reader
            .sample_info(self.video_track_id, self.current_sample_id)
            .map_err(|err| err.to_string())
    }
}

pub fn open(path: PathBuf) -> Result<File> {
    let file = fs::File::open(path)?;
    let size = file.metadata()?.len();

    let mut buf_reader = BufReader::new(file);

    let reader = Reader::read(&mut buf_reader, size)?;

    let video_track_id = reader
        .tracks()
        .iter()
        .find(|(_, track)| matches!(track.track_type(), Ok(TrackType::Video)))
        .map(|(id, _)| *id)
        .ok_or(Error::InvalidData("video track not found"))?;

    Ok(File {
        buf_reader,
        reader,
        video_track_id,
        current_sample_id: 1,
        converted: None,
    })
}

impl file::SampleReader for File {
    fn codec(&self) -> mstu_media::Codec {
        self.reader.tracks[&self.video_track_id]
            .media_format()
            .unwrap()
    }

    fn peek_sample(&mut self) -> std::result::Result<Option<SampleInfo>, String> {
        // Odd prefix lengths grow on the way to Annex-B, so those samples are
        // converted into a buffer of the reader's own first.
        if !self.reader.reads_in_place(self.video_track_id) {
            return self.peek_converted();
        }

        self.reader
            .sample_info(self.video_track_id, self.current_sample_id)
            .map_err(|err| err.to_string())
    }

    fn read_sample(&mut self, into: &mut [u8]) -> std::result::Result<(), String> {
        if let Some(converted) = self.converted.take() {
            into.copy_from_slice(&converted);
            self.current_sample_id += 1;

            return Ok(());
        }

        self.reader
            .read_sample_into(
                &mut self.buf_reader,
                self.video_track_id,
                self.current_sample_id,
                into,
            )
            .map_err(|err| err.to_string())?;

        self.current_sample_id += 1;

        Ok(())
    }

    fn timescale(&self) -> u32 {
        self.reader.tracks[&self.video_track_id].timescale()
    }

    fn duration(&self) -> Option<u64> {
        Some(self.reader.tracks[&self.video_track_id].media_duration())
    }

    fn rewind(&mut self) -> std::result::Result<(), String> {
        self.converted = None;
        self.current_sample_id = 1;
        Ok(())
    }

    fn seek(&mut self, time: u64) -> bool {
        let track = &self.reader.tracks[&self.video_track_id];

        self.current_sample_id = track
            .sync_sample_at_or_before(track.sample_at_time(time))
            .max(1);

        true
    }
}
