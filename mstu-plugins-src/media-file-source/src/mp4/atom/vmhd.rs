use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vmhd {
    pub version: u8,
    pub flags: u32,
    pub graphics_mode: u16,
    pub op_color: RgbColor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbColor {
    pub r: u16,
    pub g: u16,
    pub b: u16,
}

impl Vmhd {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let graphics_mode = reader.read_u16::<BigEndian>()?;
        let op_color = RgbColor {
            r: reader.read_u16::<BigEndian>()?,
            g: reader.read_u16::<BigEndian>()?,
            b: reader.read_u16::<BigEndian>()?,
        };

        atom::read_skip_bytes_to(reader, start + size)?;
        Ok(Vmhd {
            version: full_header.version,
            flags: full_header.flags,
            graphics_mode,
            op_color,
        })
    }
}
