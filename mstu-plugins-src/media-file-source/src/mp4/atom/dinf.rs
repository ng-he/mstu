use std::{io::Seek, vec};

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::{
    atom::{HEADER_EXT_SIZE, HEADER_SIZE},
    *,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dinf {
    dref: Dref,
}

impl Dinf {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut dref = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "dinf box contains a box with a larger size than it",
                ));
            }

            match name {
                atom::constant_name::DREF => {
                    dref = Some(Dref::read(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    atom::read_skip(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        if dref.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::DREF));
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Dinf {
            dref: dref.unwrap(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dref {
    pub version: u8,
    pub flags: u32,
    pub url: Option<Url>,
}

impl Dref {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut current = reader.stream_position()?;

        let full_header = atom::read_full_header(reader)?;
        let end = start + size;

        let mut url = None;

        let entry_count = reader.read_u32::<BigEndian>()?;
        for _i in 0..entry_count {
            if current >= end {
                break;
            }

            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "dinf box contains a box with a larger size than it",
                ));
            }

            match name {
                atom::constant_name::URL_ => {
                    url = Some(Url::read(reader, s)?);
                }
                _ => {
                    atom::read_skip(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Dref {
            version: full_header.version,
            flags: full_header.flags,
            url,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub version: u8,
    pub flags: u32,
    pub location: String,
}

impl Url {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        let buf_size = size
            .checked_sub(HEADER_SIZE + HEADER_EXT_SIZE)
            .ok_or(Error::InvalidData("url size too small"))?;

        let mut buf = vec![0u8; buf_size as usize];
        reader.read_exact(&mut buf)?;
        if let Some(end) = buf.iter().position(|&b| b == b'\0') {
            buf.truncate(end);
        }
        let location = String::from_utf8(buf).unwrap_or_default();

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Url {
            version: full_header.version,
            flags: full_header.flags,
            location,
        })
    }
}
