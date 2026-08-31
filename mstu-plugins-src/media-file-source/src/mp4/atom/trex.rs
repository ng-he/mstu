use std::io::Seek;

use crate::mp4::*;
use byteorder::{BigEndian, ReadBytesExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trex {
    pub version: u8,
    pub flags: u32,
    pub track_id: u32,
    pub default_sample_description_index: u32,
    pub default_sample_duration: u32,
    pub default_sample_size: u32,
    pub default_sample_flags: u32,
}

impl Trex {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let track_id = reader.read_u32::<BigEndian>()?;
        let default_sample_description_index = reader.read_u32::<BigEndian>()?;
        let default_sample_duration = reader.read_u32::<BigEndian>()?;
        let default_sample_size = reader.read_u32::<BigEndian>()?;
        let default_sample_flags = reader.read_u32::<BigEndian>()?;

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Trex {
            version: full_header.version,
            flags: full_header.flags,
            track_id,
            default_sample_description_index,
            default_sample_duration,
            default_sample_size,
            default_sample_flags,
        })
    }
}
