use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::*;
use crate::types::{Fixed2_30, FixedU8, FixedU16};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tkhd {
    pub version: u8,
    pub flags: u32,
    pub creation_time: u64,
    pub modification_time: u64,
    pub track_id: u32,
    pub duration: u64,
    pub layer: u16,
    pub alternate_group: u16,

    pub volume: FixedU8,
    pub matrix: Matrix,

    pub width: FixedU16,
    pub height: FixedU16,
}

impl Tkhd {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let (creation_time, modification_time, track_id, _, duration) = match full_header.version {
            1 => (
                reader.read_u64::<BigEndian>()?,
                reader.read_u64::<BigEndian>()?,
                reader.read_u32::<BigEndian>()?,
                reader.read_u32::<BigEndian>()?,
                reader.read_u64::<BigEndian>()?,
            ),
            0 => (
                reader.read_u32::<BigEndian>()? as u64,
                reader.read_u32::<BigEndian>()? as u64,
                reader.read_u32::<BigEndian>()?,
                reader.read_u32::<BigEndian>()?,
                reader.read_u32::<BigEndian>()? as u64,
            ),
            _ => {
                return Err(Error::InvalidData("version must be 0 or 1"));
            }
        };

        reader.read_u64::<BigEndian>()?; // reserved
        let layer = reader.read_u16::<BigEndian>()?;
        let alternate_group = reader.read_u16::<BigEndian>()?;
        let volume = FixedU8::from_raw(reader.read_u16::<BigEndian>()?);

        reader.read_u16::<BigEndian>()?; // reserved
        let matrix = Matrix {
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

        let width = FixedU16::from_raw(reader.read_u32::<BigEndian>()?);
        let height = FixedU16::from_raw(reader.read_u32::<BigEndian>()?);

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Tkhd {
            version: full_header.version,
            flags: full_header.flags,
            creation_time,
            modification_time,
            track_id,
            duration,
            layer,
            alternate_group,
            volume,
            matrix,
            width,
            height,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matrix {
    pub a: FixedU16,
    pub b: FixedU16,
    pub u: Fixed2_30,

    pub c: FixedU16,
    pub d: FixedU16,
    pub v: Fixed2_30,

    pub x: FixedU16,
    pub y: FixedU16,
    pub w: Fixed2_30,
}

impl std::fmt::Display for Matrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:#x} {:#x} {:#x} {:#x} {:#x} {:#x} {:#x} {:#x} {:#x}",
            self.a, self.b, self.u, self.c, self.d, self.v, self.x, self.y, self.w
        )
    }
}

impl Default for Matrix {
    fn default() -> Self {
        Self {
            // unity matrix according to ISO/IEC 14496-12:2005(E)
            a: FixedU16::from_raw(0x00010000),
            b: FixedU16::from_raw(0),
            u: Fixed2_30::from_raw(0),

            c: FixedU16::from_raw(0),
            d: FixedU16::from_raw(0x00010000),
            v: Fixed2_30::from_raw(0),

            x: FixedU16::from_raw(0),
            y: FixedU16::from_raw(0),
            w: Fixed2_30::from_raw(0x40000000),
        }
    }
}
