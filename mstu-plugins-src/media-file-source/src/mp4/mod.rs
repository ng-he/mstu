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
    file::{self, Sample},
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
}

pub struct File {
    buf_reader: BufReader<fs::File>,
    reader: Reader,
    video_track_id: u32,
    current_sample_id: u32,
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
    })
}

impl file::SampleReader for File {
    fn codec(&self) -> mstu_media::Codec {
        self.reader.tracks[&self.video_track_id]
            .media_format()
            .unwrap()
    }

    fn next_sample(&mut self) -> std::prelude::v1::Result<Option<Sample>, String> {
        match self.reader.read_sample(
            &mut self.buf_reader,
            self.video_track_id,
            self.current_sample_id,
        ) {
            Ok(Some(sample)) => {
                self.current_sample_id += 1;
                Ok(Some(sample))
            }

            Ok(None) => Ok(None),
            Err(err) => Err(err.to_string()),
        }
    }
}
