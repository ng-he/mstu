use std::io::Seek;

use crate::mp4::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mdia {
    pub mdhd: atom::Mdhd,
    pub hdlr: atom::Hdlr,
    pub minf: atom::Minf,
}

impl Mdia {
    pub fn read<R: Read + Seek>(reader: &mut R, size: u64) -> Result<Self> {
        let start = atom::read_position(reader)?;

        let mut mdhd = None;
        let mut hdlr = None;
        let mut minf = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let header = atom::read_header(reader)?;
            let atom::Header { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "mdia box contains a box with a larger size than it",
                ));
            }

            match name {
                atom::constant_name::MDHD => {
                    mdhd = Some(atom::Mdhd::read(reader, s)?);
                }
                atom::constant_name::HDLR => {
                    hdlr = Some(atom::Hdlr::read(reader, s)?);
                }
                atom::constant_name::MINF => {
                    minf = Some(atom::Minf::read(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    atom::read_skip(reader, s)?;
                }
            }

            current = reader.stream_position()?;
        }

        if mdhd.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::MDHD));
        }
        if hdlr.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::HDLR));
        }
        if minf.is_none() {
            return Err(Error::AtomNotFound(atom::constant_name::MINF));
        }

        atom::read_skip_bytes_to(reader, start + size)?;

        Ok(Mdia {
            mdhd: mdhd.unwrap(),
            hdlr: hdlr.unwrap(),
            minf: minf.unwrap(),
        })
    }
}
