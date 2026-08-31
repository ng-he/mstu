use std::io::Seek;

use byteorder::{BigEndian, ReadBytesExt};

use crate::{mp4::*, types::FixedI8};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Smhd {
    pub version: u8,
    pub flags: u32,
    pub balance: FixedI8,
}

impl Smhd {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;
        let full_header = atom::read_full_header(reader)?;
        let balance = FixedI8::from_raw(reader.read_i16::<BigEndian>()?);

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Smhd {
            version: full_header.version,
            flags: full_header.flags,
            balance,
        })
    }
}
