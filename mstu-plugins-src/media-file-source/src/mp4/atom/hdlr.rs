use std::{
    io::{Read, Seek},
    vec,
};

use byteorder::{BigEndian, ReadBytesExt};

use crate::mp4::{
    Result,
    atom::{self, FourCC, HEADER_EXT_SIZE, HEADER_SIZE},
    error::Error,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hdlr {
    pub version: u8,
    pub flags: u32,
    pub handler_type: FourCC,
    pub name: String,
}

impl Hdlr {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let full_header = atom::read_full_header(reader)?;

        reader.read_u32::<BigEndian>()?; // pre-defined
        let handler = reader.read_u32::<BigEndian>()?;

        atom::read_skip_bytes(reader, 12)?; // reserved

        let buf_size = size
            .checked_sub(HEADER_SIZE + HEADER_EXT_SIZE + 20)
            .ok_or(Error::InvalidData("hdlr size too small"))?;

        let mut buf = vec![0u8; buf_size as usize];
        reader.read_exact(&mut buf)?;
        if let Some(end) = buf.iter().position(|&b| b == b'\0') {
            buf.truncate(end);
        }
        let handler_string = String::from_utf8(buf).unwrap_or_default();

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Hdlr {
            version: full_header.version,
            flags: full_header.flags,
            handler_type: FourCC::from(handler.to_be_bytes()),
            name: handler_string,
        })
    }
}
