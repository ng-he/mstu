use crate::mp4::{
    Result,
    atom::{self, FourCC, HEADER_SIZE},
    error::Error,
};

use std::io::{Read, Seek, SeekFrom};

use byteorder::{BigEndian, ReadBytesExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Meta {
    Mdir {
        ilst: Option<atom::Ilst>,
    },
    Unknown {
        hdlr: atom::Hdlr,
        data: Vec<(FourCC, Vec<u8>)>,
    },
}

impl Meta {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let extended_header = reader.read_u32::<BigEndian>()?;
        if extended_header != 0 {
            // ISO mp4 requires this header (version & flags) to be 0. Some
            // files skip the extended header and directly start the hdlr box.
            let possible_hdlr = FourCC::from(reader.read_u32::<BigEndian>()?.to_be_bytes());
            if possible_hdlr == atom::constant_name::HDLR {
                // This file skipped the extended header! Go back to start.
                reader.seek(SeekFrom::Current(-8))?;
            } else {
                // Looks like we actually have a bad version number or flags.
                let v = (extended_header >> 24) as u8;
                return Err(Error::UnsupportedAtomVersion(atom::constant_name::META, v));
            }
        }

        let mut current = reader.stream_position()?;
        let end = start + size;

        let content_start = current;

        // find the hdlr
        let mut hdlr = None;
        while current < end {
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;

            match name {
                atom::constant_name::HDLR => {
                    hdlr = Some(atom::Hdlr::read(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    atom::read_skip(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        let Some(hdlr) = hdlr else {
            return Err(Error::AtomNotFound(atom::constant_name::HDLR));
        };

        // rewind and handle the other boxes
        reader.seek(SeekFrom::Start(content_start))?;
        current = reader.stream_position()?;

        let mut ilst = None;

        match hdlr.handler_type {
            atom::constant_name::MDIR => {
                while current < end {
                    let header: atom::Header = atom::read_header(reader)?;
                    let atom::Header { name, size: s } = header;

                    match name {
                        atom::constant_name::ILST => {
                            ilst = Some(atom::Ilst::read(reader, s)?);
                        }
                        _ => {
                            // XXX warn!()
                            atom::read_skip(reader, s)?;
                        }
                    }

                    current = reader.stream_position()?;
                }

                Ok(Meta::Mdir { ilst })
            }
            _ => {
                let mut data = Vec::new();

                while current < end {
                    let header: atom::Header = atom::read_header(reader)?;
                    let atom::Header { name, size: s } = header;

                    match name {
                        atom::constant_name::HDLR => {
                            atom::read_skip(reader, s)?;
                        }
                        _ => {
                            let mut box_data = vec![0; (s - HEADER_SIZE) as usize];
                            reader.read_exact(&mut box_data)?;

                            data.push((name, box_data));
                        }
                    }

                    current = reader.stream_position()?;
                }

                Ok(Meta::Unknown { hdlr, data })
            }
        }
    }
}
