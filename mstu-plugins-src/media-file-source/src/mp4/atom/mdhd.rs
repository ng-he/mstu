use std::{
    char::{REPLACEMENT_CHARACTER, decode_utf16},
    io::Seek,
};

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mdhd {
    pub version: u8,
    pub flags: u32,
    pub creation_time: u64,
    pub modification_time: u64,
    pub timescale: u32,
    pub duration: u64,
    pub language: String,
}

impl Mdhd {
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

        let language_code = reader.read_u16::<BigEndian>()?;
        let language = language_string(language_code);

        atom::read_skip_bytes_to(reader, start + size)?;
        Ok(Mdhd {
            version: full_header.version,
            flags: full_header.flags,
            creation_time,
            modification_time,
            timescale,
            duration,
            language,
        })
    }
}

fn language_string(language: u16) -> String {
    let mut lang: [u16; 3] = [0; 3];

    lang[0] = ((language >> 10) & 0x1F) + 0x60;
    lang[1] = ((language >> 5) & 0x1F) + 0x60;
    lang[2] = ((language) & 0x1F) + 0x60;

    // Decode utf-16 encoded bytes into a string.
    let lang_str = decode_utf16(lang.iter().cloned())
        .map(|r| r.unwrap_or(REPLACEMENT_CHARACTER))
        .collect::<String>();

    lang_str
}
