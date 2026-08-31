use std::io::{Read, Seek};

use byteorder::{BigEndian, ReadBytesExt};

use crate::{
    mp4::{
        Result,
        atom::{self, tkhd},
        error::Error,
    },
    types::{Fixed2_30, FixedU8, FixedU16},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mvhd {
    pub version: u8,
    pub flags: u32,
    pub creation_time: u64,
    pub modification_time: u64,
    pub timescale: u32,
    pub duration: u64,

    pub rate: FixedU16,
    pub volume: FixedU8,
    pub matrix: tkhd::Matrix,

    pub next_track_id: u32,
}

impl Mvhd {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let (creation_time, modification_time, timescale, duration) = match full_header.version {
            1 => (
                reader.read_u64::<BigEndian>()?,
                reader.read_u64::<BigEndian>()?,
                reader.read_u32::<BigEndian>()?,
                reader.read_u64::<BigEndian>()?,
            ),
            0 => (
                reader.read_u32::<BigEndian>()? as u64,
                reader.read_u32::<BigEndian>()? as u64,
                reader.read_u32::<BigEndian>()?,
                reader.read_u32::<BigEndian>()? as u64,
            ),
            _ => {
                return Err(Error::InvalidData("version must be 0 or 1"));
            }
        };

        let rate = FixedU16::from_raw(reader.read_u32::<BigEndian>()?);
        let volume = FixedU8::from_raw(reader.read_u16::<BigEndian>()?);

        reader.read_u16::<BigEndian>()?; // reserved = 0
        reader.read_u64::<BigEndian>()?; // reserved = 0

        let matrix = tkhd::Matrix {
            a: FixedU16::from_raw(reader.read_u32::<BigEndian>()?),
            b: FixedU16::from_raw(reader.read_u32::<BigEndian>()?),
            u: Fixed2_30::from_raw(reader.read_i32::<BigEndian>()?),

            c: FixedU16::from_raw(reader.read_u32::<BigEndian>()?),
            d: FixedU16::from_raw(reader.read_u32::<BigEndian>()?),
            v: Fixed2_30::from_raw(reader.read_i32::<BigEndian>()?),

            x: FixedU16::from_raw(reader.read_u32::<BigEndian>()?),
            y: FixedU16::from_raw(reader.read_u32::<BigEndian>()?),
            w: Fixed2_30::from_raw(reader.read_i32::<BigEndian>()?),
        };

        atom::read_skip_bytes(reader, 24)?; // pre_defined = 0

        let next_track_id = reader.read_u32::<BigEndian>()?;

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Self {
            version: full_header.version,
            flags: full_header.flags,
            creation_time,
            modification_time,
            timescale,
            duration,
            rate,
            volume,
            matrix,
            next_track_id,
        })
    }
}
