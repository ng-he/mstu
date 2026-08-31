use std::io::Seek;

use crate::mp4::*;
use byteorder::{BigEndian, ReadBytesExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mehd {
    pub version: u8,
    pub flags: u32,
    pub fragment_duration: u64,
}

impl Mehd {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let fragment_duration = match full_header.version {
            1 => reader.read_u64::<BigEndian>()?,
            0 => reader.read_u32::<BigEndian>()? as u64,
            _ => return Err(Error::InvalidData("version must be 0 or 1")),
        };

        atom::read_skip_bytes_to(reader, start + size)?;
        Ok(Mehd {
            version: full_header.version,
            flags: full_header.flags,
            fragment_duration,
        })
    }
}
